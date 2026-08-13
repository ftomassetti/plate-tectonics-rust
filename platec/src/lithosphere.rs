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
use std::cmp::Reverse;
use std::collections::BinaryHeap;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

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

/// Cost of a plate front advancing one cell through oceanic crust. Thin and
/// weak, so fronts race across it.
const GROWTH_COST_OCEAN: u32 = 1;

/// Cost of advancing one cell into continental crust, plus a further
/// [`GROWTH_COST_PER_THICKNESS`] per unit of crust above `CONTINENTAL_BASE`.
/// Fronts crawl across continents, so a landmass is usually claimed whole by
/// whichever plate reaches it first and boundaries settle in the ocean.
const GROWTH_COST_LAND: u32 = 10;
const GROWTH_COST_PER_THICKNESS: f32 = 6.0;
const GROWTH_COST_MAX: u32 = 40;

/// Continental crust older than this counts as cratonic: the ancient, cold,
/// thick cores that on Earth survive intact through several supercontinent
/// cycles. `iter_count` restarts each cycle while `amap` does not, so crust
/// carried over from an earlier cycle wraps to a huge age — which is exactly
/// the "very old" answer we want.
const CRATON_AGE: u32 = 200;

/// Cost of advancing into a craton. Not infinite, so growth always terminates,
/// but high enough that a front will go the long way round rather than cut one.
const GROWTH_COST_CRATON: u32 = 200;

/// How many times seed selection will redraw looking for oceanic crust before
/// accepting whatever it has. Rifts nucleate in thin, weak lithosphere; they do
/// not open through the middle of a craton.
const SEED_OCEAN_ATTEMPTS: u32 = 8;

/// Whether divergent boundaries are refilled with new crust.
const BOOL_REGENERATE_CRUST: u32 = 1;
// The buoyancy sweep is folded into the regeneration sweep, which only covers
// the whole map while this is 1.
const _: () = assert!(BOOL_REGENERATE_CRUST == 1);

/// How often plate rectangles are refitted to their contents, in iterations.
/// The refit costs one linear read of each plate's box, so amortising it over a
/// handful of steps makes it free while keeping the boxes near-tight — plates
/// only grow in multiples of 8, and only when crust lands outside.
const COMPACT_BOUNDS_PERIOD: u32 = 16;

/// Empty cells kept around a plate's crust when refitting. Crust can spread by
/// at most one cell per iteration, so a margin of one period's worth means no
/// cell that could become crust before the next refit is ever clipped off, and
/// `erode` sees the same boundary condition it would have without refitting.
const COMPACT_BOUNDS_MARGIN: u32 = COMPACT_BOUNDS_PERIOD;

/// Relative spread of the height jitter applied to freshly regenerated sea
/// floor. The buoyancy bonus steps by `BUOYANCY_BONUS_X * OCEANIC_BASE /
/// MAX_BUOYANCY_AGE` = 0.015 per age unit on a base of 0.3, so +/-10% is a
/// couple of band widths — enough to dither the iso-age edges away while
/// leaving the overall age-versus-depth gradient intact.
const REGEN_CRUST_NOISE: f32 = 0.10;

/// Height difference below which two overlapping plates count as equally
/// buoyant. Sized to the regenerated-crust jitter above, the dominant source of
/// height noise: a tie band much tighter than that would let the winner across a
/// broad overlap be decided by noise rather than by buoyancy.
const BUOYANCY_TIE: f32 = REGEN_CRUST_NOISE * OCEANIC_BASE * BUOYANCY_BONUS_X;

/// Deterministic per-cell jitter in [-1, 1).
///
/// Hashed from the location and the iteration rather than drawn from
/// `randsource`, so that adding it does not disturb the random draw order the
/// rest of the simulation depends on.
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

        // Candidate origins, drawn without replacement so that two plate
        // centres are never identical.
        let mut candidates: Vec<u32> = (0..map_area).collect();

        // Select N plate centers from the global map.
        for i in 0..self.num_plates {
            // Redraw a few times looking for oceanic crust. Continental crust
            // is thick and strong: new spreading centres open in the ocean, so
            // seeding uniformly puts rifts through the middle of continents.
            let mut pick = 0usize;
            let mut p = 0u32;
            for _ in 0..SEED_OCEAN_ATTEMPTS {
                pick = (self.randsource.next() % candidates.len() as u32) as usize;
                p = candidates[pick];
                if self.hmap[p as usize] < CONTINENTAL_BASE {
                    break;
                }
            }
            candidates.swap_remove(pick);

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

    /// Cost for a plate front to advance into the given cell.
    fn growth_cost(&self, index: usize) -> u32 {
        let h = self.hmap[index];
        if h < CONTINENTAL_BASE {
            return GROWTH_COST_OCEAN;
        }
        if self.iter_count.wrapping_sub(self.amap[index]) >= CRATON_AGE {
            return GROWTH_COST_CRATON;
        }
        let extra = ((h - CONTINENTAL_BASE) * GROWTH_COST_PER_THICKNESS) as u32;
        (GROWTH_COST_LAND + extra).min(GROWTH_COST_MAX)
    }

    /// "Grow" plates from their origins until the surface is fully populated.
    ///
    /// A multi-source Dijkstra rather than an isotropic flood fill: each front
    /// advances by cheapest accumulated cost, and crossing continental crust
    /// costs roughly ten times what crossing ocean does. Fronts therefore race
    /// around continents through the ocean and meet there, so a landmass is
    /// usually claimed whole by whichever plate reaches it first instead of
    /// being cut down the middle by an arbitrary boundary.
    ///
    /// Continents still split when two seeds land inside the same landmass,
    /// which is the intended rare case — real continents do rift apart.
    fn grow_plates(&mut self) {
        let world_width = self.world_dimension.get_width();
        let world_height = self.world_dimension.get_height();
        let map_area = self.world_dimension.get_area() as usize;

        let mut best = vec![u32::MAX; map_area];
        // (accumulated cost, cell, plate) — Reverse turns the max-heap into a
        // min-heap, and the tuple order makes ties deterministic.
        let mut heap: BinaryHeap<Reverse<(u32, u32, u32)>> = BinaryHeap::new();

        for i in 0..self.num_plates {
            let seed = self.plate_areas[i as usize].border[0];
            if best[seed as usize] == u32::MAX {
                best[seed as usize] = 0;
                self.imap[seed] = i;
                heap.push(Reverse((0, seed, i)));
            }
        }

        while let Some(Reverse((cost, p, i))) = heap.pop() {
            if cost > best[p as usize] {
                continue; // Superseded by a cheaper route.
            }

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

            for &(cell, dir) in &[(n, 0u8), (s, 1), (w, 2), (e, 3)] {
                let next = cost + self.growth_cost(cell as usize);
                if next >= best[cell as usize] {
                    continue;
                }
                best[cell as usize] = next;
                self.imap[cell] = i;
                heap.push(Reverse((next, cell, i)));

                // Extend the plate's bounding box. A newly claimed cell is
                // 4-adjacent to one already inside the box, so it is at most one
                // row or column beyond an edge.
                let area = &mut self.plate_areas[i as usize];
                match dir {
                    0 => {
                        if area.top == self.world_dimension.y_mod(top + 1) {
                            area.top = top;
                            area.hgt += 1;
                        }
                    }
                    1 => {
                        if btm == self.world_dimension.y_mod(area.btm + 1) {
                            area.btm = btm;
                            area.hgt += 1;
                        }
                    }
                    2 => {
                        if area.lft == self.world_dimension.x_mod(lft + 1) {
                            area.lft = lft;
                            area.wdt += 1;
                        }
                    }
                    _ => {
                        if rgt == self.world_dimension.x_mod(area.rgt + 1) {
                            area.rgt = rgt;
                            area.wdt += 1;
                        }
                    }
                }
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
    /// The plate's height and age are re-read after every `set_crust` call
    /// rather than cached, so that later reads see the updated values.
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

                    // The read has to be fresh each time: `set_crust` below
                    // may change it.
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

        // Where two plates overlap with near-equal height, deciding who
        // subducts by crust age shreds the plate map: age varies from cell to
        // cell across an overlap, so the winner alternates and the two plates
        // come out interleaved in thin slivers. Decide by continuity instead —
        // within the tie band, whoever held the cell last iteration keeps it —
        // so an overlap resolves as one coherent region with a stable boundary.
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
                // Only one plate left; there is nothing to remove.
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

        // Retire the current index map into `prev_imap` by swapping the two
        // buffers rather than copying: `update_height_and_plate_index_maps`
        // overwrites every cell of `imap` below, so its incoming contents (the
        // map from two steps ago) are dead. Nothing reads `imap` between here
        // and that overwrite.
        std::mem::swap(&mut self.prev_imap, &mut self.imap);

        // Realize accumulated external forces on each plate. Each plate touches
        // only itself here — its own map, bounds, segments and random source —
        // so the plates run independently and the result does not depend on the
        // order they are visited in.
        let compact = self.iter_count % COMPACT_BOUNDS_PERIOD == 0;
        let erode_now = self.erosion_period > 0 && self.iter_count % self.erosion_period == 0;
        let active = &mut self.plates[..self.num_plates as usize];
        let step_plate = |p: &mut Plate| {
            // Before `reset_segments`, which asserts that the segment buffer
            // matches the bounds area.
            if compact {
                p.compact_bounds(COMPACT_BOUNDS_MARGIN);
            }
            p.reset_segments();

            if erode_now {
                p.erode(CONTINENTAL_BASE);
            }

            p.move_plate();
        };
        #[cfg(feature = "parallel")]
        active.par_iter_mut().for_each(step_plate);
        #[cfg(not(feature = "parallel"))]
        active.iter_mut().for_each(step_plate);

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

        // Fill divergent boundaries with new crustal material, molten magma,
        // and add the buoyancy bonus.
        //
        // Cells are independent, so this runs as a set of row bands. The two
        // pieces of cross-cell state are collected per band and merged in band
        // order afterwards, which reproduces the sequential result exactly:
        // the plate tallies are order-free integer counts, and the `set_crust`
        // calls are replayed in the row-major order they were recorded in.
        // Nothing in the sweep reads the plates, so deferring those writes is
        // safe.
        let width = self.world_dimension.get_width() as usize;
        let rows = (BOOL_REGENERATE_CRUST * self.world_dimension.get_height()) as usize;
        let band_rows = 64usize.min(rows.max(1));
        let band = width * band_rows;

        let num_plates = self.num_plates;
        let iter_count = self.iter_count;

        let bands: Vec<_> = self.hmap.as_mut_slice()[..rows * width]
            .chunks_mut(band)
            .zip(self.imap.as_mut_slice()[..rows * width].chunks_mut(band))
            .zip(self.amap.as_mut_slice()[..rows * width].chunks_mut(band))
            .zip(self.prev_imap.as_slice()[..rows * width].chunks(band))
            .enumerate()
            .map(|(bi, (((h, im), am), pv))| (bi, h, im, am, pv))
            .collect();

        let run_band = |(bi, h, im, am, pv): (
            usize,
            &mut [f32],
            &mut [u32],
            &mut [u32],
            &[u32],
        )| {
            let mut found = vec![0u32; num_plates as usize];
            let mut deferred: Vec<(usize, u32, u32)> = Vec::new();
            let y0 = bi * band_rows;

            for c in 0..im.len() {
                let x = (c % width) as u32;
                let y = (y0 + c / width) as u32;

                if im[c] >= num_plates {
                    // The owner of this new crust is that neighbour plate which
                    // was located at this point before the plates moved.
                    im[c] = pv[c];

                    // If this is oceanic crust then add buoyancy to it: magma
                    // that has just crystallized into oceanic crust is more
                    // buoyant than that which has had a lot of time to cool
                    // down and become more dense.
                    am[c] = iter_count;

                    // Every cell uncovered in one iteration shares an age, and
                    // the buoyancy below turns age into height with no noise at
                    // all — so each iteration's wake came out as a hard-edged
                    // iso-height band trailing the plate. Jitter the starting
                    // height to dither those edges away.
                    h[c] = OCEANIC_BASE
                        * BUOYANCY_BONUS_X
                        * (1.0 + REGEN_CRUST_NOISE * cell_jitter(x, y, iter_count));

                    // This should probably not happen.
                    if im[c] < num_plates {
                        deferred.push((im[c] as usize, x, y));
                    }
                } else {
                    found[im[c] as usize] += 1;
                    if h[c] <= 0.0 {
                        panic!("Occupied point has no land mass!");
                    }
                }

                // Add some "virginity buoyancy" to all pixels for a visual
                // boost! :) Calculate the inverted age of this piece of crust,
                // forcing the result to be the minimum of the inverted age and
                // the max buoyancy bonus age.
                let mut crust_age = iter_count.wrapping_sub(am[c]);
                crust_age = MAX_BUOYANCY_AGE.wrapping_sub(crust_age);
                crust_age &= u32::from(crust_age <= MAX_BUOYANCY_AGE).wrapping_neg();

                h[c] += f32::from(h[c] < CONTINENTAL_BASE)
                    * BUOYANCY_BONUS_X
                    * OCEANIC_BASE
                    * crust_age as f32
                    * MULINV_MAX_BUOYANCY_AGE;
            }

            (found, deferred)
        };

        #[cfg(feature = "parallel")]
        let per_band: Vec<_> = bands.into_par_iter().map(run_band).collect();
        #[cfg(not(feature = "parallel"))]
        let per_band: Vec<_> = bands.into_iter().map(run_band).collect();

        for (found, deferred) in per_band {
            for (i, n) in found.iter().enumerate() {
                self.plate_indices_found[i] += n;
            }
            for (owner, x, y) in deferred {
                self.plates[owner].set_crust(x, y, OCEANIC_BASE, iter_count);
            }
        }

        self.remove_empty_plates();

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
