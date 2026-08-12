//! Port of `src/lithosphere.hpp` / `src/lithosphere.cpp`.
//!
//! The lithosphere is the rigid outermost shell of a rocky planet, divided into
//! several rigid areas i.e. plates. As time passes the topography of the planet
//! evolves as the result of plate dynamics. This class creates and manages all
//! the plates and updates the height map to match their current setup.

use crate::geometry::WorldDimension;
use crate::heightmap::{AgeMap, HeightMap, IndexMap};
use crate::noise::create_slow_noise;
use crate::plate::Plate;
use crate::platec_assert;
use crate::simplerandom::SimpleRandom;
use crate::world_point::WorldPoint;

pub const CONTINENTAL_BASE: f32 = 1.0;
pub const OCEANIC_BASE: f32 = 0.1;

const SUBDUCT_RATIO: f32 = 0.5;

const BUOYANCY_BONUS_X: f32 = 3.0;
const MAX_BUOYANCY_AGE: u32 = 20;
const MULINV_MAX_BUOYANCY_AGE: f32 = 1.0 / MAX_BUOYANCY_AGE as f32;

const RESTART_ENERGY_RATIO: f32 = 0.15;
const RESTART_SPEED_LIMIT: f32 = 2.0;
const RESTART_ITERATIONS: u32 = 600;
const NO_COLLISION_TIME_LIMIT: u32 = 10;

/// The C++ `#define BOOL_REGENERATE_CRUST 1`.
const BOOL_REGENERATE_CRUST: u32 = 1;

/// Relative spread of the height jitter applied to freshly regenerated sea
/// floor. The buoyancy bonus steps by `BUOYANCY_BONUS_X * OCEANIC_BASE /
/// MAX_BUOYANCY_AGE` = 0.015 per age unit on a base of 0.3, so +/-10% is a
/// couple of band widths — enough to dither the iso-age edges away while
/// leaving the overall age-versus-depth gradient intact.
#[cfg(not(feature = "classic_cpp"))]
const REGEN_CRUST_NOISE: f32 = 0.10;

/// Height difference below which two overlapping plates count as equally
/// buoyant. The C++ uses `2 * f32::EPSILON` (~2.4e-7), far tighter than any
/// physically meaningful difference at heights of order 0.1 to 1.0, so across a
/// broad overlap the winner was effectively picked by crust-age noise. This is
/// sized to the regenerated-crust jitter above, the dominant source of that
/// noise.
#[cfg(not(feature = "classic_cpp"))]
const BUOYANCY_TIE: f32 = REGEN_CRUST_NOISE * OCEANIC_BASE * BUOYANCY_BONUS_X;

/// Deterministic per-cell jitter in [-1, 1).
///
/// Hashed from the location and the iteration rather than drawn from
/// `randsource`, so that adding it does not disturb the random draw order the
/// rest of the port depends on.
#[cfg(not(feature = "classic_cpp"))]
fn cell_jitter(x: u32, y: u32, t: u32) -> f32 {
    let mut h = x
        .wrapping_mul(0x9E37_79B1)
        ^ y.wrapping_mul(0x85EB_CA77)
        ^ t.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2545_F491);
    h ^= h >> 13;
    // Top 24 bits -> [0, 1) -> [-1, 1).
    ((h >> 8) as f32 / (1u32 << 24) as f32) * 2.0 - 1.0
}

/// Errors the C++ signals by throwing `std::runtime_error`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatecError {
    InvalidDimensions(String),
}

impl std::fmt::Display for PlatecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlatecError::InvalidDimensions(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for PlatecError {}

/// Wrapper for growing a plate from a seed; contains the plate's dimensions.
/// Used exclusively in the plate creation phase.
#[derive(Clone, Debug, Default)]
struct PlateArea {
    /// The plate's unprocessed border pixels.
    border: Vec<u32>,
    /// Most bottom pixel of the plate.
    btm: u32,
    /// Most left pixel of the plate.
    lft: u32,
    /// Most right pixel of the plate.
    rgt: u32,
    /// Most top pixel of the plate.
    top: u32,
    /// Width of the area in pixels.
    wdt: u32,
    /// Height of the area in pixels.
    hgt: u32,
}

/// Container for collision details between two plates.
#[derive(Clone, Copy, Debug)]
struct PlateCollision {
    /// Index of the other plate involved in the event.
    index: u32,
    /// Coordinates of the collision in world space.
    wx: u32,
    wy: u32,
    /// Amount of crust that will deform/subduct.
    crust: f32,
}

impl PlateCollision {
    fn new(index: u32, wx: u32, wy: u32, crust: f32) -> Self {
        platec_assert!(crust >= 0.0, "Crust must be a positive value");
        Self {
            index,
            wx,
            wy,
            crust,
        }
    }
}

pub struct Lithosphere {
    /// Height map representing the topography of the system.
    hmap: HeightMap,
    /// Plate index map of the "owner" of each map point.
    imap: IndexMap,
    /// Plate index map from the last update.
    prev_imap: IndexMap,
    /// Age map of the system's surface (topography).
    amap: AgeMap,
    /// The plates that constitute the system.
    plates: Vec<Plate>,
    plate_areas: Vec<PlateArea>,
    /// Used in the update loop to remove plates.
    plate_indices_found: Vec<u32>,

    /// Number of overlapping pixels that triggers aggregation.
    aggr_overlap_abs: u32,
    /// Percentage of overlapping area that triggers aggregation.
    aggr_overlap_rel: f32,
    /// Number of times the system has been restarted.
    cycle_count: u32,
    /// Number of iterations between global erosion.
    erosion_period: u32,
    /// Percentage of overlapping crust that is folded.
    folding_ratio: f32,
    /// Iteration count, used to timestamp new crust.
    iter_count: u32,
    /// Maximum number of times the system will be restarted.
    max_cycles: u32,
    /// Number of plates in the initial setting.
    max_plates: u32,
    /// Number of plates in the current setting.
    num_plates: u32,

    collisions: Vec<Vec<PlateCollision>>,
    subductions: Vec<Vec<PlateCollision>>,

    /// Max total kinetic energy in the system so far.
    peak_ek: f32,
    /// Iterations since the last continental collision.
    last_coll_count: u32,

    world_dimension: WorldDimension,
    randsource: SimpleRandom,
    steps: i32,
}

/// Two simultaneous mutable borrows out of the plate vector, needed by
/// `aggregateCrust` / `collide` which operate on a pair of plates.
fn two_mut(plates: &mut [Plate], a: usize, b: usize) -> (&mut Plate, &mut Plate) {
    assert_ne!(a, b, "cannot borrow the same plate twice");
    if a < b {
        let (left, right) = plates.split_at_mut(b);
        (&mut left[a], &mut right[0])
    } else {
        let (left, right) = plates.split_at_mut(a);
        (&mut right[0], &mut left[b])
    }
}

impl Lithosphere {
    /// Initialise the system's height map, i.e. its topography.
    ///
    /// * `sea_level` — amount of surface area that becomes oceanic crust.
    /// * `erosion_period` — number of iterations between global erosion.
    /// * `folding_ratio` — percentage of overlapping crust that is folded.
    /// * `aggr_ratio_abs` — number of overlapping points causing aggregation.
    /// * `aggr_ratio_rel` — percentage of overlapping area causing aggregation.
    /// * `num_cycles` — number of times the system will be restarted.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        seed: u32,
        width: u32,
        height: u32,
        mut sea_level: f32,
        erosion_period: u32,
        folding_ratio: f32,
        aggr_ratio_abs: u32,
        aggr_ratio_rel: f32,
        num_cycles: u32,
        max_plates: u32,
    ) -> Result<Self, PlatecError> {
        if width < 5 || height < 5 {
            return Err(PlatecError::InvalidDimensions(
                "Width and height should be >=5".to_string(),
            ));
        }

        let world_dimension = WorldDimension::new(width, height);
        let mut lito = Self {
            hmap: HeightMap::new(width, height),
            imap: IndexMap::new(width, height),
            prev_imap: IndexMap::new(width, height),
            amap: AgeMap::new(width, height),
            plates: Vec::new(),
            plate_areas: vec![PlateArea::default(); max_plates as usize],
            plate_indices_found: vec![0; max_plates as usize],
            aggr_overlap_abs: aggr_ratio_abs,
            aggr_overlap_rel: aggr_ratio_rel,
            cycle_count: 0,
            erosion_period,
            folding_ratio,
            iter_count: 0,
            max_cycles: num_cycles,
            max_plates,
            num_plates: 0,
            collisions: vec![Vec::new(); max_plates as usize],
            subductions: vec![Vec::new(); max_plates as usize],
            peak_ek: 0.0,
            last_coll_count: 0,
            world_dimension,
            randsource: SimpleRandom::new(seed),
            steps: 0,
        };

        let tmp_dim = WorldDimension::new(width + 1, height + 1);
        let a = tmp_dim.get_area() as usize;
        let mut tmp = vec![0.0f32; a];

        // The generator is passed **by value**, so `self.randsource` is not
        // advanced by the noise generation.
        create_slow_noise(&mut tmp, &tmp_dim, lito.randsource);

        let mut lowest = tmp[0];
        let mut highest = tmp[0];
        for &v in tmp.iter().take(a).skip(1) {
            lowest = if lowest < v { lowest } else { v };
            highest = if highest > v { highest } else { v };
        }

        // Scale to [0 ... 1].
        for v in tmp.iter_mut() {
            *v = (*v - lowest) / (highest - lowest);
        }

        let mut sea_threshold = 0.5f32;
        let mut th_step = 0.5f32;

        // Find the actual value in the height map that produces the
        // continent-sea ratio defined by "sea_level".
        while th_step > 0.01 {
            let mut count = 0u32;
            for &v in tmp.iter() {
                count += u32::from(v < sea_threshold);
            }

            th_step *= 0.5;
            if count as f32 / (a as f32) < sea_level {
                sea_threshold += th_step;
            } else {
                sea_threshold -= th_step;
            }
        }

        sea_level = sea_threshold;
        // Genesis 1:9-10.
        for v in tmp.iter_mut() {
            *v = f32::from(*v > sea_level) * (*v + CONTINENTAL_BASE)
                + f32::from(*v <= sea_level) * OCEANIC_BASE;
        }

        // Scalp the +1 away from the map side to get a power-of-two side
        // length! Practically only the redundant map edges are removed.
        for y in 0..height {
            let dst = world_dimension.line_index(y) as usize;
            let src = tmp_dim.line_index(y) as usize;
            let w = width as usize;
            lito.hmap.as_mut_slice()[dst..dst + w].copy_from_slice(&tmp[src..src + w]);
        }

        for area in lito.plate_areas.iter_mut() {
            area.border.reserve(8);
        }
        lito.create_plates();

        Ok(lito)
    }

    /// Split the current topography into the given number of (rigid) plates.
    /// Any previous set of plates is discarded.
    pub fn create_plates(&mut self) {
        let map_area = self.world_dimension.get_area();
        self.num_plates = self.max_plates;

        // Initialise the "free plate center position" lookup table. This way
        // two plate centers will never be identical.
        for i in 0..map_area {
            self.imap[i] = i;
        }

        // Select N plate centers from the global map.
        for i in 0..self.num_plates {
            // Randomly select an unused plate origin.
            let p = self.imap[self.randsource.next() % (map_area - i)];
            let y = self.world_dimension.y_from_index(p);
            let x = self.world_dimension.x_from_index(p);

            let area = &mut self.plate_areas[i as usize];
            area.lft = x; // Save the origin...
            area.rgt = x;
            area.top = y;
            area.btm = y;
            area.wdt = 1;
            area.hgt = 1;

            area.border.clear();
            area.border.push(p); // ...and mark it as border.

            // Overwrite the used entry with the last unused entry in the array.
            self.imap[p] = self.imap[map_area - i - 1];
        }

        self.imap.set_all(0xFFFF_FFFF);

        self.grow_plates();

        // Check that all the points of the map are owned.
        for i in 0..map_area {
            platec_assert!(
                self.imap[i] < self.num_plates,
                "A point was not assigned to any plate"
            );
        }

        // Extract and create plates from the initial terrain.
        self.plates.clear();
        for i in 0..self.num_plates {
            let (x0, y0, width, height) = {
                let area = &mut self.plate_areas[i as usize];
                area.wdt = self.world_dimension.x_cap(area.wdt);
                area.hgt = self.world_dimension.y_cap(area.hgt);

                let x0 = area.lft;
                let x1 = 1 + x0 + area.wdt;
                let y0 = area.top;
                let y1 = 1 + y0 + area.hgt;
                (x0, y0, x1 - x0, y1 - y0)
            };

            let mut pmap = vec![0.0f32; (width * height) as usize];

            // Copy the plate's height data from the global map into the local map.
            let mut j = 0usize;
            for y in y0..y0 + height {
                for x in x0..x0 + width {
                    let k = self.world_dimension.normalized_index_of(x, y);
                    pmap[j] = self.hmap[k] * f32::from(self.imap[k] == i);
                    j += 1;
                }
            }

            // Create the plate. The pmap buffer becomes owned by the plate.
            let seed = self.randsource.next();
            self.plates.push(Plate::new(
                seed,
                pmap,
                width,
                height,
                x0,
                y0,
                i,
                self.world_dimension,
            ));
        }

        self.iter_count = self.num_plates + MAX_BUOYANCY_AGE;
        self.peak_ek = 0.0;
        self.last_coll_count = 0;
    }

    /// "Grow" plates from their origins until the surface is fully populated.
    fn grow_plates(&mut self) {
        let world_width = self.world_dimension.get_width();
        let world_height = self.world_dimension.get_height();
        let mut max_border = 1u32;

        while max_border != 0 {
            max_border = 0;
            for i in 0..self.num_plates {
                let n_border = self.plate_areas[i as usize].border.len() as u32;
                max_border = if max_border > n_border {
                    max_border
                } else {
                    n_border
                };

                if n_border == 0 {
                    continue;
                }
                let j = (self.randsource.next() % n_border) as usize;
                let p = self.plate_areas[i as usize].border[j];
                let cy = self.world_dimension.y_from_index(p);
                let cx = self.world_dimension.x_from_index(p);

                let lft = if cx > 0 { cx - 1 } else { world_width - 1 };
                let rgt = if cx < world_width - 1 { cx + 1 } else { 0 };
                let top = if cy > 0 { cy - 1 } else { world_height - 1 };
                let btm = if cy < world_height - 1 { cy + 1 } else { 0 };

                let n = top * world_width + cx; // North.
                let s = btm * world_width + cx; // South.
                let w = cy * world_width + lft; // West.
                let e = cy * world_width + rgt; // East.

                if self.imap[n] >= self.num_plates {
                    self.imap[n] = i;
                    let area = &mut self.plate_areas[i as usize];
                    area.border.push(n);

                    if area.top == self.world_dimension.y_mod(top + 1) {
                        area.top = top;
                        area.hgt += 1;
                    }
                }

                if self.imap[s] >= self.num_plates {
                    self.imap[s] = i;
                    let area = &mut self.plate_areas[i as usize];
                    area.border.push(s);

                    if btm == self.world_dimension.y_mod(area.btm + 1) {
                        area.btm = btm;
                        area.hgt += 1;
                    }
                }

                if self.imap[w] >= self.num_plates {
                    self.imap[w] = i;
                    let area = &mut self.plate_areas[i as usize];
                    area.border.push(w);

                    if area.lft == self.world_dimension.x_mod(lft + 1) {
                        area.lft = lft;
                        area.wdt += 1;
                    }
                }

                if self.imap[e] >= self.num_plates {
                    self.imap[e] = i;
                    let area = &mut self.plate_areas[i as usize];
                    area.border.push(e);

                    if rgt == self.world_dimension.x_mod(area.rgt + 1) {
                        area.rgt = rgt;
                        area.wdt += 1;
                    }
                }

                // Overwrite the processed point with an unprocessed one.
                let area = &mut self.plate_areas[i as usize];
                let last = *area.border.last().unwrap();
                area.border[j] = last;
                area.border.pop();
            }
        }
    }

    fn clear_plates(&mut self) {
        self.plates.clear();
        self.num_plates = 0;
    }

    // -----------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------

    pub fn get_cycle_count(&self) -> u32 {
        self.cycle_count
    }
    pub fn get_iteration_count(&self) -> u32 {
        self.iter_count
    }
    pub fn get_world_dimension(&self) -> &WorldDimension {
        &self.world_dimension
    }
    pub fn get_plate_count(&self) -> u32 {
        self.num_plates
    }
    pub fn get_age_map(&self) -> &[u32] {
        self.amap.as_slice()
    }
    pub fn get_topography(&self) -> &[f32] {
        self.hmap.as_slice()
    }
    pub fn get_plates_map(&self) -> &[u32] {
        self.imap.as_slice()
    }
    pub fn get_width(&self) -> u32 {
        self.world_dimension.get_width()
    }
    pub fn get_height(&self) -> u32 {
        self.world_dimension.get_height()
    }
    pub fn is_finished(&self) -> bool {
        self.get_plate_count() == 0
    }
    pub fn get_plate(&self, index: u32) -> &Plate {
        platec_assert!(index < self.num_plates, "invalid plate index");
        &self.plates[index as usize]
    }
    pub fn get_steps(&self) -> i32 {
        self.steps
    }

    #[allow(dead_code)]
    fn random_position(&mut self) -> WorldPoint {
        let x = self.randsource.next() % self.world_dimension.get_width();
        let y = self.randsource.next() % self.world_dimension.get_height();
        WorldPoint::new(x, y, &self.world_dimension)
    }

    /// At least two plates are at the same location: move some crust from the
    /// SMALLER plate onto the LARGER one.
    ///
    /// Note that the C++ passes `this_map` / `this_age` as live pointers into
    /// plate `i`'s buffers, so values read after a `setCrust` call see the
    /// updated data. Reading them freshly here preserves that.
    #[allow(clippy::too_many_arguments)]
    fn resolve_juxtapositions(
        &mut self,
        i: u32,
        j: usize,
        k: u32,
        x_mod: u32,
        y_mod: u32,
        continental_collisions: &mut u32,
    ) {
        platec_assert!(i < self.num_plates, "Given invalid plate index");
        let k_us = k as usize;

        // Record collisions to both plates. This also creates a continent
        // segment at the collided location in both plates.
        let owner = self.imap[k_us];
        let this_area = self.plates[i as usize].add_collision(x_mod, y_mod);
        let prev_area = self.plates[owner as usize].add_collision(x_mod, y_mod);

        if this_area < prev_area {
            let this_map_j = self.plates[i as usize].get_map().0[j];
            let this_age_j = self.plates[i as usize].get_map().1[j];
            let coll = PlateCollision::new(owner, x_mod, y_mod, this_map_j * self.folding_ratio);

            // Give some...
            self.hmap[k_us] += coll.crust;
            let h = self.hmap[k_us];
            self.plates[owner as usize].set_crust(x_mod, y_mod, h, this_age_j);

            // And take some.
            self.plates[i as usize].set_crust(
                x_mod,
                y_mod,
                this_map_j * (1.0 - self.folding_ratio),
                this_age_j,
            );

            // Add the collision to the earlier plate's list.
            self.collisions[i as usize].push(coll);
            *continental_collisions += 1;
        } else {
            let coll = PlateCollision::new(i, x_mod, y_mod, self.hmap[k_us] * self.folding_ratio);

            let this_map_j = self.plates[i as usize].get_map().0[j];
            let amap_k = self.amap[k_us];
            self.plates[i as usize].set_crust(x_mod, y_mod, this_map_j + coll.crust, amap_k);

            let h = self.hmap[k_us];
            self.plates[owner as usize].set_crust(
                x_mod,
                y_mod,
                h * (1.0 - self.folding_ratio),
                amap_k,
            );

            self.collisions[owner as usize].push(coll);
            *continental_collisions += 1;

            // Give the location to the larger plate.
            self.hmap[k_us] = self.plates[i as usize].get_map().0[j];
            self.imap[k_us] = i;
            self.amap[k_us] = self.plates[i as usize].get_map().1[j];
        }
    }

    /// Update the height and plate index maps.
    ///
    /// Doing it plate by plate is much faster than doing it index-wise: each
    /// plate's map memory area is accessed sequentially and only once.
    fn update_height_and_plate_index_maps(
        &mut self,
        oceanic_collisions: &mut u32,
        continental_collisions: &mut u32,
    ) {
        let world_width = self.world_dimension.get_width();
        let world_height = self.world_dimension.get_height();
        self.hmap.set_all(0.0);
        self.imap.set_all(0xFFFF_FFFF);

        for i in 0..self.num_plates {
            let iu = i as usize;
            let x0 = self.plates[iu].get_left_as_uint();
            let y0 = self.plates[iu].get_top_as_uint();
            let x1 = x0 + self.plates[iu].get_width();
            let y1 = y0 + self.plates[iu].get_height();

            let x_mod_start = (x0 + world_width) % world_width;
            let mut y_mod = (y0 + world_height) % world_height;

            // These loops are ugly, but using modulus in here is a hog.
            let mut j = 0usize;
            for _y in y0..y1 {
                let y_width = y_mod * world_width;
                let mut x_mod = x_mod_start;

                for _x in x0..x1 {
                    let k = (x_mod + y_width) as usize;

                    // The read has to be fresh each time: `setCrust` below may
                    // change it, and the C++ reads through a live pointer.
                    let this_map_j = self.plates[iu].get_map().0[j];

                    if this_map_j >= 2.0 * f32::EPSILON {
                        self.step_point(
                            i,
                            j,
                            k,
                            x_mod,
                            y_mod,
                            oceanic_collisions,
                            continental_collisions,
                        );
                    }

                    j += 1;
                    x_mod += 1;
                    if x_mod >= world_width {
                        x_mod -= world_width;
                    }
                }

                y_mod += 1;
                if y_mod >= world_height {
                    y_mod -= world_height;
                }
            }
        }
    }

    /// The body of the innermost loop of `updateHeightAndPlateIndexMaps`.
    #[allow(clippy::too_many_arguments)]
    fn step_point(
        &mut self,
        i: u32,
        j: usize,
        k: usize,
        x_mod: u32,
        y_mod: u32,
        oceanic_collisions: &mut u32,
        continental_collisions: &mut u32,
    ) {
        let iu = i as usize;

        if self.imap[k] >= self.num_plates {
            // No one here yet: this plate becomes the "owner" of the current
            // location as the first plate to have crust on it.
            self.hmap[k] = self.plates[iu].get_map().0[j];
            self.imap[k] = i;
            self.amap[k] = self.plates[iu].get_map().1[j];
            return;
        }

        // DO NOT ACCEPT HEIGHT EQUALITY! Equality leads to subduction of a
        // shore that's barely above sea level. It's a lot less serious a
        // problem to treat very shallow waters as continent...
        let this_map_j = self.plates[iu].get_map().0[j];
        let prev_is_oceanic = self.hmap[k] < CONTINENTAL_BASE;
        let this_is_oceanic = this_map_j < CONTINENTAL_BASE;

        let owner = self.imap[k];
        let prev_timestamp = self.plates[owner as usize].get_crust_timestamp(x_mod, y_mod);
        let this_timestamp = self.plates[iu].get_map().1[j];

        #[cfg(feature = "classic_cpp")]
        let prev_is_buoyant = (self.hmap[k] > this_map_j)
            || ((self.hmap[k] + 2.0 * f32::EPSILON > this_map_j)
                && (self.hmap[k] < 2.0 * f32::EPSILON + this_map_j)
                && (prev_timestamp >= this_timestamp));

        // Where two plates overlap with near-equal height, the C++ decides who
        // subducts by crust age. Age varies from cell to cell across an overlap,
        // so the winner alternates and the plate map comes out shredded into
        // thin interleaved slivers of both plates. Decide by continuity instead:
        // within the tie band, whoever held the cell last iteration keeps it, so
        // an overlap resolves as one coherent region with a stable boundary.
        #[cfg(not(feature = "classic_cpp"))]
        let prev_is_buoyant = if (self.hmap[k] - this_map_j).abs() <= BUOYANCY_TIE {
            if self.prev_imap[k] == owner {
                true
            } else if self.prev_imap[k] == i {
                false
            } else {
                prev_timestamp >= this_timestamp
            }
        } else {
            self.hmap[k] > this_map_j
        };

        // Handle subduction of oceanic crust as a special case.
        if this_is_oceanic && prev_is_buoyant {
            // This plate will be the subducting one. The level of effect that
            // subduction has is directly related to the amount of water on top
            // of the subducting plate.
            let sediment =
                SUBDUCT_RATIO * OCEANIC_BASE * (CONTINENTAL_BASE - this_map_j) / CONTINENTAL_BASE;

            // Save the collision to the receiving plate's list.
            let coll = PlateCollision::new(i, x_mod, y_mod, sediment);
            self.subductions[owner as usize].push(coll);
            *oceanic_collisions += 1;

            // Remove the subducted oceanic lithosphere from the plate. This is
            // crucial for a) having the correct amount of colliding crust and
            // b) protecting subducted locations from receiving crust from other
            // subductions/collisions.
            self.plates[iu].set_crust(x_mod, y_mod, this_map_j - OCEANIC_BASE, this_timestamp);

            if self.plates[iu].get_map().0[j] <= 0.0 {
                return; // Nothing more to collide.
            }
        } else if prev_is_oceanic {
            let sediment =
                SUBDUCT_RATIO * OCEANIC_BASE * (CONTINENTAL_BASE - self.hmap[k]) / CONTINENTAL_BASE;

            let coll = PlateCollision::new(owner, x_mod, y_mod, sediment);
            self.subductions[iu].push(coll);
            *oceanic_collisions += 1;

            let h = self.hmap[k];
            self.plates[owner as usize].set_crust(x_mod, y_mod, h - OCEANIC_BASE, prev_timestamp);
            self.hmap[k] -= OCEANIC_BASE;

            if self.hmap[k] <= 0.0 {
                self.imap[k] = i;
                self.hmap[k] = self.plates[iu].get_map().0[j];
                self.amap[k] = self.plates[iu].get_map().1[j];
                return;
            }
        }

        self.resolve_juxtapositions(i, j, k as u32, x_mod, y_mod, continental_collisions);
    }

    fn update_collisions(&mut self) {
        for i in 0..self.num_plates {
            let iu = i as usize;
            let colls = std::mem::take(&mut self.collisions[iu]);
            for coll in colls.iter() {
                platec_assert!(i != coll.index, "when colliding: SRC == DEST!");

                // Collision causes friction. Apply it to both plates.
                self.plates[iu].apply_friction(coll.crust);
                self.plates[coll.index as usize].apply_friction(coll.crust);

                let (coll_count_i, coll_ratio_i) =
                    self.plates[iu].get_collision_info(coll.wx, coll.wy);
                let (coll_count_j, coll_ratio_j) =
                    self.plates[coll.index as usize].get_collision_info(coll.wx, coll.wy);

                // Find the minimum count of collisions between two continents
                // on different plates. It's the minimum because a large plate
                // will get collisions from all over whereas a smaller plate
                // will get just a few — and it's those few that matter between
                // these two plates.
                let mut coll_count = coll_count_i;
                coll_count = coll_count.wrapping_sub(
                    coll_count.wrapping_sub(coll_count_j)
                        & u32::from(coll_count > coll_count_j).wrapping_neg(),
                );

                // Find the maximum amount of collided surface area between two
                // continents on different plates. As above, it's the
                // "experience" of the smaller plate that matters here.
                let mut coll_ratio = coll_ratio_i;
                coll_ratio +=
                    (coll_ratio_j - coll_ratio) * f32::from(coll_ratio_j > coll_ratio);

                if (coll_count > self.aggr_overlap_abs) | (coll_ratio > self.aggr_overlap_rel) {
                    let amount = {
                        let (a, b) = two_mut(&mut self.plates, iu, coll.index as usize);
                        a.aggregate_crust(b, coll.wx, coll.wy)
                    };

                    // Calculate the new direction and speed for the merged
                    // plate system, that is, for the receiving plate!
                    let (b, a) = two_mut(&mut self.plates, coll.index as usize, iu);
                    b.collide(a, amount);
                }
            }

            self.collisions[iu].clear();
        }
    }

    /// Remove empty plates from the system.
    fn remove_empty_plates(&mut self) {
        let mut i = 0i64;
        while (i as u32) < self.num_plates {
            let iu = i as usize;
            if self.num_plates == 1 {
                // The C++ prints "ONLY ONE PLATE LEFT!" here every iteration.
            } else if self.plate_indices_found[iu] == 0 {
                self.plates.swap_remove(iu);
                self.plate_indices_found[iu] =
                    self.plate_indices_found[(self.num_plates - 1) as usize];

                // Life is seldom as simple as it seems at first: replace the
                // moved plate's index in the index map to match its current
                // position in the array!
                for j in 0..self.world_dimension.get_area() as usize {
                    if self.imap[j] == self.num_plates - 1 {
                        self.imap[j] = i as u32;
                    }
                }

                self.num_plates -= 1;
                i -= 1;
            }
            i += 1;
        }
    }

    /// Simulate one step of plate tectonics.
    pub fn update(&mut self) {
        self.steps += 1;
        let mut total_velocity = 0.0f32;
        let mut system_kinetic_energy = 0.0f32;

        for i in 0..self.num_plates as usize {
            total_velocity += self.plates[i].get_velocity();
            system_kinetic_energy += self.plates[i].get_momentum();
        }

        if system_kinetic_energy > self.peak_ek {
            self.peak_ek = system_kinetic_energy;
        }

        // If there have been no continental collisions during past iterations
        // then interesting activity has ceased and we should restart. Also if
        // the simulation has been going on for too long already, restart,
        // because the interesting stuff has most likely ended.
        if total_velocity < RESTART_SPEED_LIMIT
            || system_kinetic_energy / self.peak_ek < RESTART_ENERGY_RATIO
            || self.last_coll_count > NO_COLLISION_TIME_LIMIT
            || self.iter_count > RESTART_ITERATIONS
        {
            self.restart();
            return;
        }

        let map_area = self.world_dimension.get_area();
        // Keep a copy of the previous index map.
        self.prev_imap.copy_from(&self.imap);

        // Realize accumulated external forces on each plate.
        for i in 0..self.num_plates as usize {
            self.plates[i].reset_segments();

            if self.erosion_period > 0 && self.iter_count % self.erosion_period == 0 {
                self.plates[i].erode(CONTINENTAL_BASE);
            }

            self.plates[i].move_plate();
        }

        let mut oceanic_collisions = 0u32;
        let mut continental_collisions = 0u32;

        self.update_height_and_plate_index_maps(
            &mut oceanic_collisions,
            &mut continental_collisions,
        );

        // Update the counter of iterations since the last continental collision.
        self.last_coll_count = self
            .last_coll_count
            .wrapping_add(1)
            & u32::from(continental_collisions == 0).wrapping_neg();

        for i in 0..self.num_plates as usize {
            let subs = std::mem::take(&mut self.subductions[i]);
            for coll in subs.iter() {
                platec_assert!(i as u32 != coll.index, "when subducting: SRC == DEST!");

                // Do not apply friction to oceanic plates. This is a very cheap
                // way to emulate slab pull: just perform subduction and on our
                // way we go!
                let (vel_x, vel_y) = {
                    let other = &self.plates[coll.index as usize];
                    (other.get_vel_x(), other.get_vel_y())
                };
                let iter_count = self.iter_count;
                self.plates[i].add_crust_by_subduction(
                    coll.wx, coll.wy, coll.crust, iter_count, vel_x, vel_y,
                );
            }

            self.subductions[i].clear();
        }

        self.update_collisions();

        self.plate_indices_found.iter_mut().for_each(|v| *v = 0);

        // Fill divergent boundaries with new crustal material, molten magma.
        let mut i = 0usize;
        for y in 0..BOOL_REGENERATE_CRUST * self.world_dimension.get_height() {
            for x in 0..self.world_dimension.get_width() {
                if self.imap[i] >= self.num_plates {
                    // The owner of this new crust is that neighbour plate which
                    // was located at this point before the plates moved.
                    self.imap[i] = self.prev_imap[i];

                    // If this is oceanic crust then add buoyancy to it: magma
                    // that has just crystallized into oceanic crust is more
                    // buoyant than that which has had a lot of time to cool
                    // down and become more dense.
                    self.amap[i] = self.iter_count;

                    // Every cell uncovered in one iteration shares an age, and
                    // the buoyancy pass below turns age into height with no
                    // noise at all — so each iteration's wake came out as a
                    // hard-edged iso-height band trailing the plate. Jitter the
                    // starting height to dither those edges away.
                    #[cfg(not(feature = "classic_cpp"))]
                    {
                        self.hmap[i] = OCEANIC_BASE
                            * BUOYANCY_BONUS_X
                            * (1.0 + REGEN_CRUST_NOISE * cell_jitter(x, y, self.iter_count));
                    }
                    #[cfg(feature = "classic_cpp")]
                    {
                        self.hmap[i] = OCEANIC_BASE * BUOYANCY_BONUS_X;
                    }

                    // This should probably not happen.
                    if self.imap[i] < self.num_plates {
                        let owner = self.imap[i] as usize;
                        let iter_count = self.iter_count;
                        self.plates[owner].set_crust(x, y, OCEANIC_BASE, iter_count);
                    }
                } else {
                    let owner = self.imap[i] as usize;
                    self.plate_indices_found[owner] += 1;
                    if self.hmap[i] <= 0.0 {
                        panic!("Occupied point has no land mass!");
                    }
                }
                i += 1;
            }
        }

        self.remove_empty_plates();

        // Add some "virginity buoyancy" to all pixels for a visual boost! :)
        for i in 0..map_area as usize {
            // Calculate the inverted age of this piece of crust, forcing the
            // result to be the minimum of the inverted age and the max buoyancy
            // bonus age.
            let mut crust_age = self.iter_count.wrapping_sub(self.amap[i]);
            crust_age = MAX_BUOYANCY_AGE.wrapping_sub(crust_age);
            crust_age &= u32::from(crust_age <= MAX_BUOYANCY_AGE).wrapping_neg();

            self.hmap[i] += f32::from(self.hmap[i] < CONTINENTAL_BASE)
                * BUOYANCY_BONUS_X
                * OCEANIC_BASE
                * crust_age as f32
                * MULINV_MAX_BUOYANCY_AGE;
        }

        self.iter_count += 1;
    }

    /// Replace the plates with a new population.
    fn restart(&mut self) {
        let map_area = self.world_dimension.get_area();

        // No increment if running forever.
        self.cycle_count += u32::from(self.max_cycles > 0);
        if self.cycle_count > self.max_cycles {
            return;
        }

        // Update the height map to include all recent changes.
        self.hmap.set_all(0.0);
        for i in 0..self.num_plates as usize {
            let x0 = self.plates[i].get_left_as_uint();
            let y0 = self.plates[i].get_top_as_uint();
            let x1 = x0 + self.plates[i].get_width();
            let y1 = y0 + self.plates[i].get_height();

            // Copy the first part of the plate onto the world map.
            let mut j = 0usize;
            for y in y0..y1 {
                for x in x0..x1 {
                    let x_mod = self.world_dimension.x_mod(x);
                    let y_mod = self.world_dimension.y_mod(y);
                    let idx = self.world_dimension.index_of(x_mod, y_mod) as usize;
                    let h0 = self.hmap[idx];
                    let (this_map, this_age) = self.plates[i].get_map();
                    let h1 = this_map[j];
                    let a1 = this_age[j];
                    let a0 = self.amap[idx];

                    let h_sum = h0 + h1;
                    // Avoid division by zero: if both heights are zero, use the
                    // new age.
                    self.amap[idx] = if h_sum > 0.0 {
                        ((h0 * a0 as f32 + h1 * a1 as f32) / h_sum) as u32
                    } else {
                        a1
                    };
                    self.hmap[idx] += h1;
                    j += 1;
                }
            }
        }

        // Clear the plate array.
        self.clear_plates();

        // Create new plates IFF there are cycles left to run! However, if the
        // max cycle count is "ETERNITY" then 0 < 0 + 1 always.
        if self.cycle_count < self.max_cycles + u32::from(self.max_cycles == 0) {
            self.create_plates();

            // Restore the ages of the plates' points of crust!
            for i in 0..self.num_plates as usize {
                let x0 = self.plates[i].get_left_as_uint();
                let y0 = self.plates[i].get_top_as_uint();
                let x1 = x0 + self.plates[i].get_width();
                let y1 = y0 + self.plates[i].get_height();

                let mut j = 0usize;
                for y in y0..y1 {
                    for x in x0..x1 {
                        let x_mod = self.world_dimension.x_mod(x);
                        let y_mod = self.world_dimension.y_mod(y);
                        let idx = self.world_dimension.index_of(x_mod, y_mod) as usize;
                        let age = self.amap[idx];
                        self.plates[i].age_map_mut()[j] = age;
                        j += 1;
                    }
                }
            }

            return;
        }

        // Add some "virginity buoyancy" to all pixels for a visual boost.
        for i in 0..map_area as usize {
            let mut crust_age = self.iter_count.wrapping_sub(self.amap[i]);
            crust_age = MAX_BUOYANCY_AGE.wrapping_sub(crust_age);
            crust_age &= u32::from(crust_age <= MAX_BUOYANCY_AGE).wrapping_neg();

            self.hmap[i] += f32::from(self.hmap[i] < CONTINENTAL_BASE)
                * BUOYANCY_BONUS_X
                * OCEANIC_BASE
                * crust_age as f32
                * MULINV_MAX_BUOYANCY_AGE;
        }
    }
}
