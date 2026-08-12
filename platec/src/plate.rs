//! A single tectonic plate: its crust, its motion and its interactions.

use crate::bounds::Bounds;
use crate::geometry::{Dimension, FloatPoint, FloatVector, WorldDimension};
use crate::heightmap::{AgeMap, HeightMap};
use crate::mass::{Mass, MassBuilder, MassLike};
use crate::movement::{Movement, MovementLike};
use crate::plate_functions::{calculate_crust, CrustNeighbours};
use crate::platec_assert;
use crate::rectangle::BAD_INDEX;
use crate::segments::{ContinentId, SegmentCtx, Segments, SegmentsApi};
use crate::simplerandom::SimpleRandom;

pub struct Plate {
    world_dimension: WorldDimension,
    randsource: SimpleRandom,
    /// Bitmap of the plate's structure/height.
    map: HeightMap,
    /// Bitmap of the age of the plate's soil: timestamp of creation.
    age_map: AgeMap,
    bounds: Bounds,
    mass: Mass,
    movement: Movement,
    /// Boxed so that tests can inject a mock, as `plate::injectSegments` does.
    /// `Send` so that the per-plate work can be spread across threads.
    segments: Box<dyn SegmentsApi + Send>,
}

impl Plate {
    /// Initialise a plate with the supplied height map.
    ///
    /// * `m` — the height map of the terrain (ownership is taken, matching the
    ///   buffer).
    /// * `w`, `h` — width and height of the height map in pixels.
    /// * `x`, `y` — position of the height map's left-top corner on the world map.
    ///
    /// Initialisation order matters: the random source is seeded first and then
    /// passed **by value** to `Movement`, so the plate's own generator is not
    /// advanced by the movement's two draws.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        seed: u32,
        m: Vec<f32>,
        w: u32,
        h: u32,
        x: u32,
        y: u32,
        plate_age: u32,
        world_dimension: WorldDimension,
    ) -> Self {
        let plate_area = w.wrapping_mul(h);
        // Only the first `w * h` elements are used, so callers are free to
        // hand over a larger buffer. Trim rather than assert.
        let mut m = m;
        m.truncate(plate_area as usize);
        let randsource = SimpleRandom::new(seed);
        let mass = MassBuilder::from_slice(&m, &Dimension::new(w, h)).build();
        let movement = Movement::new(randsource, world_dimension);
        let map = HeightMap::from_vec(m, w, h);
        let mut age_map = AgeMap::new(w, h);
        let bounds = Bounds::new(
            world_dimension,
            FloatPoint::new(x as f32, y as f32),
            Dimension::new(w, h),
        );

        let mut k = 0usize;
        for y in 0..bounds.height() {
            for x in 0..bounds.width() {
                // Set the age of ALL points in this plate to the same value.
                // The right thing to do would be to simulate the generation of
                // new oceanic crust as if the plate had been moving in its
                // current direction until all the plate's (oceanic) crust
                // received an age.
                age_map.set(x, y, plate_age & u32::from(map[k] > 0.0).wrapping_neg());
                k += 1;
            }
        }

        Self {
            world_dimension,
            randsource,
            map,
            age_map,
            bounds,
            mass,
            movement,
            segments: Box::new(Segments::new(plate_area)),
        }
    }

    /// Increment the collision counter of the continent at the given location.
    ///
    /// Returns the surface area of the collided continent (HACK!).
    pub fn add_collision(&mut self, wx: u32, wy: u32) -> u32 {
        let id = self.continent_id_at(wx, wy);
        let seg = self.segments.seg_mut(id);
        seg.inc_coll_count();
        seg.area()
    }

    /// Add crust to the plate as the result of a continental collision.
    ///
    /// * `x`, `y` — location of the new crust on the global world map.
    /// * `z` — amount of crust to add.
    /// * `time` — time of creation of the new crust.
    /// * `active_continent` — segment ID of the continent being processed.
    pub fn add_crust_by_collision(
        &mut self,
        x: u32,
        y: u32,
        z: f32,
        time: u32,
        active_continent: ContinentId,
    ) {
        // Add crust, extending the plate if necessary.
        let crust = self.get_crust(x, y);
        self.set_crust(x, y, crust + z, time);

        let mut lx = x;
        let mut ly = y;
        let index = self.bounds.get_valid_map_index(&mut lx, &mut ly);
        self.segments.set_id(index, active_continent);

        let data = self.segments.seg_mut(active_continent);
        data.inc_area();
        data.enlarge_to_contain(lx, ly);
    }

    /// Simulate subduction of an oceanic plate under this plate.
    ///
    /// Subduction is simulated by calculating the distance on the surface that
    /// subducting sediment will travel under the plate until the subducting
    /// slab has reached a certain depth where the heat triggers the melting and
    /// uprising of molten magma.
    pub fn add_crust_by_subduction(
        &mut self,
        x: u32,
        y: u32,
        z: f32,
        t: u32,
        mut dx: f32,
        mut dy: f32,
    ) {
        let mut lx = x;
        let mut ly = y;
        self.bounds.get_valid_map_index(&mut lx, &mut ly);

        // Take the vector difference only between plates that move more or less
        // in the same direction. This makes the subduction direction behave
        // better.
        let dot = self.movement.dot(dx, dy);
        let sign = if dot > 0.0 { 1.0f32 } else { 0.0f32 };
        dx -= self.movement.velocity_on_x_len(sign);
        dy -= self.movement.velocity_on_y_len(sign);

        // Four draws in a fixed order — do not reorder.
        let mut offset = self.randsource.next_float();
        let offset_sign = (2 * (self.randsource.next() % 2) as i32 - 1) as f32;
        offset *= offset * offset * offset_sign;
        let mut offset2 = self.randsource.next_float();
        let offset_sign2 = (2 * (self.randsource.next() % 2) as i32 - 1) as f32;
        offset2 *= offset2 * offset2 * offset_sign2;
        dx = 10.0 * dx + 3.0 * offset;
        dy = 10.0 * dy + 3.0 * offset2;

        let fx = lx as f32 + dx;
        let fy = ly as f32 + dy;

        if self.bounds.is_in_limits(fx, fy) {
            let index = self.bounds.index(fx as u32, fy as u32) as usize;
            if self.map[index] > 0.0 {
                let t = ((self.map[index] * self.age_map[index] as f32 + z * t as f32)
                    / (self.map[index] + z)) as u32;
                self.age_map[index] = (t as f32 * f32::from(z > 0.0)) as u32;

                self.map[index] += z;
                self.mass.inc_mass(z);
            }
        }
    }

    /// Add continental crust from this plate as part of another plate.
    ///
    /// Aggregation of two continents is the event where the collided pieces of
    /// crust fuse together at the point of collision. It is crucial to merge
    /// not only the collided pieces of crust but the entire continent that is
    /// part of the colliding bit of crust — without merging the entire plate
    /// and all those continental pieces that have NOTHING to do with the
    /// collision in question.
    ///
    /// Returns the amount of crust aggregated to the destination plate.
    pub fn aggregate_crust(&mut self, p: &mut Plate, wx: u32, wy: u32) -> f32 {
        let mut lx = wx;
        let mut ly = wy;
        let index = self.bounds.get_valid_map_index(&mut lx, &mut ly);

        let seg_id = self.segments.id(index);

        // This check forces the caller to do things in the proper order!
        //
        // Continents usually collide at several locations simultaneously, so if
        // the segment being merged now were removed from the segmentation
        // bookkeeping, the next point of collision processed during the same
        // iteration step would cause a premature abort. Therefore the
        // bookkeeping is left intact; it causes no significant problems because
        // all crust is cleared and empty points are not processed at all.
        //
        // One continent may have many points of collision. If one of them
        // causes the continent to aggregate then all successive collisions and
        // attempts of aggregation would necessarily change nothing at all,
        // because the continent was removed from this plate earlier!
        if self.segments.seg(seg_id).is_empty() {
            return 0.0; // Do not process empty continents.
        }

        let active_continent = p.select_collision_segment(wx, wy);

        // Wrap coordinates around world edges to safeguard subtractions.
        let wx = wx.wrapping_add(self.world_dimension.get_width());
        let wy = wy.wrapping_add(self.world_dimension.get_height());

        let old_mass = self.mass.get_mass();

        // Add all of the collided continent's crust to the destination plate.
        let (top, bottom, left, right) = {
            let seg = self.segments.seg(seg_id);
            (
                seg.get_top(),
                seg.get_bottom(),
                seg.get_left(),
                seg.get_right(),
            )
        };
        for y in top..=bottom {
            for x in left..=right {
                let i = y.wrapping_mul(self.bounds.width()).wrapping_add(x);
                if self.segments.id(i) == seg_id && self.map[i as usize] > 0.0 {
                    p.add_crust_by_collision(
                        wx.wrapping_add(x).wrapping_sub(lx),
                        wy.wrapping_add(y).wrapping_sub(ly),
                        self.map[i as usize],
                        self.age_map[i as usize],
                        active_continent,
                    );

                    self.mass.inc_mass(-1.0 * self.map[i as usize]);
                    self.map[i as usize] = 0.0;
                }
            }
        }

        // Mark the segment as non-existent.
        self.segments.seg_mut(seg_id).mark_non_existent();
        old_mass - self.mass.get_mass()
    }

    /// Decrease the speed of the plate relative to its total mass.
    pub fn apply_friction(&mut self, deformed_mass: f32) {
        // Remove the energy that deformation consumed from the plate's kinetic
        // energy: F - dF = ma - dF => a = dF/m.
        if !self.mass.null() {
            self.movement.apply_friction(deformed_mass, self.mass.get_mass());
        }
    }

    /// Collide two plates according to Newton's laws of motion.
    pub fn collide(&mut self, p: &mut Plate, coll_mass: f32) {
        if !self.mass.null() && coll_mass > 0.0 {
            self.movement.collide(&self.mass, p, coll_mass);
        }
    }

    /// Visible for testing.
    pub fn calculate_crust(&self, x: u32, y: u32, index: u32) -> CrustNeighbours {
        calculate_crust(
            x,
            y,
            index,
            &self.world_dimension,
            &self.map,
            self.bounds.width(),
            self.bounds.height(),
        )
    }

    fn find_river_sources(&self, lower_bound: f32, sources: &mut Vec<u32>) {
        let bounds_height = self.bounds.height();
        let bounds_width = self.bounds.width();

        // Find all tops.
        for y in 0..bounds_height {
            let y_width = y * bounds_width;
            for x in 0..bounds_width {
                let index = y_width + x;

                if self.map[index as usize] < lower_bound {
                    continue;
                }

                let c = self.calculate_crust(x, y, index);

                // This location is either at the edge of the plate or it is not
                // the tallest of its neighbours: don't start a river here.
                if c.w_crust * c.e_crust * c.n_crust * c.s_crust == 0.0 {
                    continue;
                }

                sources.push(index);
            }
        }
    }

    fn flow_rivers(&self, lower_bound: f32, sources: &mut Vec<u32>, tmp: &mut HeightMap) {
        let bounds_area = self.bounds.area() as usize;
        let mut sinks_data: Vec<u32> = Vec::new();
        let mut flow_done = vec![false; bounds_area];

        let mut sources = std::mem::take(sources);
        let sinks = &mut sinks_data;
        let sources_ref = &mut sources;

        // From each top, start flowing water along the steepest slope.
        while !sources_ref.is_empty() {
            while let Some(index) = sources_ref.pop() {
                let y = index / self.bounds.width();
                let x = index - y * self.bounds.width();

                if self.map[index as usize] < lower_bound {
                    continue;
                }

                let c = self.calculate_crust(x, y, index);
                let (mut w_crust, mut e_crust, mut n_crust, mut s_crust) =
                    (c.w_crust, c.e_crust, c.n_crust, c.s_crust);

                // If this is the lowest part of its neighbourhood, stop.
                if w_crust + e_crust + n_crust + s_crust == 0.0 {
                    continue;
                }

                w_crust += f32::from(w_crust == 0.0) * self.map[index as usize];
                e_crust += f32::from(e_crust == 0.0) * self.map[index as usize];
                n_crust += f32::from(n_crust == 0.0) * self.map[index as usize];
                s_crust += f32::from(s_crust == 0.0) * self.map[index as usize];

                // Find the lowest neighbour.
                let mut lowest_crust = w_crust;
                let mut dest = index.wrapping_sub(1);

                if e_crust < lowest_crust {
                    lowest_crust = e_crust;
                    dest = index.wrapping_add(1);
                }

                if n_crust < lowest_crust {
                    lowest_crust = n_crust;
                    dest = index.wrapping_sub(self.bounds.width());
                }

                if s_crust < lowest_crust {
                    lowest_crust = s_crust;
                    dest = index.wrapping_add(self.bounds.width());
                }
                let _ = lowest_crust;

                // If it's not handled yet, add it as a new sink.
                if dest < self.bounds.area() && !flow_done[dest as usize] {
                    sinks.push(dest);
                    flow_done[dest as usize] = true;
                }

                // Erode this location with the water flow.
                tmp[index as usize] -= (tmp[index as usize] - lower_bound) * 0.2;
            }

            std::mem::swap(sources_ref, sinks);
            sinks.clear();
        }
    }

    /// Apply the plate-wide erosion algorithm. The plate's total mass and the
    /// centre of mass are updated.
    ///
    /// * `lower_bound` — limit below which there is no erosion.
    pub fn erode(&mut self, lower_bound: f32) {
        let mut sources: Vec<u32> = Vec::new();

        let mut tmp_hm = self.map.clone();
        self.find_river_sources(lower_bound, &mut sources);
        self.flow_rivers(lower_bound, &mut sources, &mut tmp_hm);

        // Add random noise (10 %) to the heightmap.
        for i in 0..self.bounds.area() as usize {
            let alpha = 0.2 * self.randsource.next_float();
            tmp_hm[i] += 0.1 * tmp_hm[i] - alpha * tmp_hm[i];
            // Clamp to zero to prevent floating point errors from accumulating
            // and causing negative mass values (Issue #30).
            if tmp_hm[i] < 0.0 {
                tmp_hm[i] = 0.0;
            }
        }

        self.map.copy_from(&tmp_hm);
        tmp_hm.set_all(0.0);
        let mut mass_builder = MassBuilder::new();

        for y in 0..self.bounds.height() {
            for x in 0..self.bounds.width() {
                let index = (y * self.bounds.width() + x) as usize;
                mass_builder.add_point(x, y, self.map[index]);
                // Careful not to overwrite earlier amounts.
                tmp_hm[index] += self.map[index];

                if self.map[index] < lower_bound {
                    continue;
                }

                let c = self.calculate_crust(x, y, index as u32);
                let (w_crust, e_crust, n_crust, s_crust) =
                    (c.w_crust, c.e_crust, c.n_crust, c.s_crust);
                let (w, e, n, s) = (
                    c.w as usize,
                    c.e as usize,
                    c.n as usize,
                    c.s as usize,
                );

                // This location has no neighbours (ARTIFACT!) or it is the
                // lowest part of its area. Either way the work here is done.
                if w_crust + e_crust + n_crust + s_crust == 0.0 {
                    continue;
                }

                // The steeper the slope, the more water flows along it; the more
                // downhill sources, the more water flows to here.
                //
                // Calculate the difference in height between this point and the
                // neighbours that are lower than it.
                let w_diff = self.map[index] - w_crust;
                let e_diff = self.map[index] - e_crust;
                let n_diff = self.map[index] - n_crust;
                let s_diff = self.map[index] - s_crust;

                let mut min_diff = w_diff;
                min_diff -= (min_diff - e_diff) * f32::from(e_diff < min_diff);
                min_diff -= (min_diff - n_diff) * f32::from(n_diff < min_diff);
                min_diff -= (min_diff - s_diff) * f32::from(s_diff < min_diff);

                // Calculate the sum of the difference between the lower
                // neighbours and the TALLEST lower neighbour.
                let diff_sum = (w_diff - min_diff) * f32::from(w_crust > 0.0)
                    + (e_diff - min_diff) * f32::from(e_crust > 0.0)
                    + (n_diff - min_diff) * f32::from(n_crust > 0.0)
                    + (s_diff - min_diff) * f32::from(s_crust > 0.0);

                platec_assert!(diff_sum >= 0.0, "Difference sum must be positive");

                if diff_sum < min_diff {
                    // There's NOT enough room in the neighbours to contain all
                    // the crust from this peak so that it would be as tall as
                    // its tallest lower neighbour. So the first step is to make
                    // ALL lower neighbours and this point equally tall.
                    tmp_hm[w] += (w_diff - min_diff) * f32::from(w_crust > 0.0);
                    tmp_hm[e] += (e_diff - min_diff) * f32::from(e_crust > 0.0);
                    tmp_hm[n] += (n_diff - min_diff) * f32::from(n_crust > 0.0);
                    tmp_hm[s] += (s_diff - min_diff) * f32::from(s_crust > 0.0);
                    tmp_hm[index] -= min_diff;

                    min_diff -= diff_sum;

                    // Spread the remaining crust equally among all lower
                    // neighbours.
                    min_diff /= 1.0
                        + f32::from(w_crust > 0.0)
                        + f32::from(e_crust > 0.0)
                        + f32::from(n_crust > 0.0)
                        + f32::from(s_crust > 0.0);

                    tmp_hm[w] += min_diff * f32::from(w_crust > 0.0);
                    tmp_hm[e] += min_diff * f32::from(e_crust > 0.0);
                    tmp_hm[n] += min_diff * f32::from(n_crust > 0.0);
                    tmp_hm[s] += min_diff * f32::from(s_crust > 0.0);
                    tmp_hm[index] += min_diff;
                } else {
                    let unit = min_diff / diff_sum;

                    // Remove all crust from this location, making it as tall as
                    // its tallest lower neighbour.
                    tmp_hm[index] -= min_diff;

                    // Spread all removed crust among the other lower neighbours.
                    tmp_hm[w] += unit * (w_diff - min_diff) * f32::from(w_crust > 0.0);
                    tmp_hm[e] += unit * (e_diff - min_diff) * f32::from(e_crust > 0.0);
                    tmp_hm[n] += unit * (n_diff - min_diff) * f32::from(n_crust > 0.0);
                    tmp_hm[s] += unit * (s_diff - min_diff) * f32::from(s_crust > 0.0);
                }
            }
        }

        // Clamp all heightmap values to prevent negative mass from floating
        // point errors (Issue #30).
        for i in 0..self.bounds.area() as usize {
            if tmp_hm[i] < 0.0 {
                tmp_hm[i] = 0.0;
            }
        }

        self.map.copy_from(&tmp_hm);
        self.mass = mass_builder.build();
    }

    /// Retrieve collision statistics of the continent at the given location:
    /// the count of collisions and the % of area that collided.
    pub fn get_collision_info(&mut self, wx: u32, wy: u32) -> (u32, f32) {
        let id = self.continent_id_at(wx, wy);
        let seg = self.segments.seg(id);
        let count = seg.coll_count();
        // +1 avoids a division by zero.
        let ratio = seg.coll_count() as f32 / (1 + seg.area()) as f32;
        (count, ratio)
    }

    /// Retrieve the surface area of the continent lying at the desired location.
    pub fn get_continent_area(&self, wx: u32, wy: u32) -> u32 {
        let mut lx = wx;
        let mut ly = wy;
        let index = self.bounds.get_valid_map_index(&mut lx, &mut ly);
        platec_assert!(
            self.segments.id(index) < self.segments.size(),
            "Segment index invalid"
        );
        self.segments.seg(self.segments.id(index)).area()
    }

    /// Get the amount of the plate's crustal material at some location.
    pub fn get_crust(&self, x: u32, y: u32) -> f32 {
        let mut lx = x;
        let mut ly = y;
        let index = self.bounds.get_map_index(&mut lx, &mut ly);
        if index != BAD_INDEX {
            self.map[index as usize]
        } else {
            0.0
        }
    }

    /// Get the timestamp of the plate's crustal material at some location.
    /// Zero is returned if the location contains no crust.
    pub fn get_crust_timestamp(&self, x: u32, y: u32) -> u32 {
        let mut lx = x;
        let mut ly = y;
        let index = self.bounds.get_map_index(&mut lx, &mut ly);
        if index != BAD_INDEX {
            self.age_map[index as usize]
        } else {
            0
        }
    }

    /// The crust height map and the crust timestamp map.
    pub fn get_map(&self) -> (&[f32], &[u32]) {
        (self.map.as_slice(), self.age_map.as_slice())
    }

    /// Move the plate along its trajectory.
    pub fn move_plate(&mut self) {
        self.movement.move_plate();

        // Location modulations into the range [0..world width/height[ are a
        // must! If left undone SOMETHING WILL BREAK DOWN SOMEWHERE in the code.
        self.bounds.shift(
            self.movement.velocity_on_x(),
            self.movement.velocity_on_y(),
        );
    }

    /// Clear any earlier continental crust partitions.
    ///
    /// The plate keeps internal bookkeeping of distinct areas of continental
    /// crust for a more realistic collision response. As the number of
    /// collisions grows the bookkeeping becomes less and less accurate, finally
    /// resulting in striking artefacts, so callers are given a way to reset it.
    /// Refit the plate's rectangle to the crust it actually holds.
    ///
    /// `set_crust` grows the rectangle in multiples of 8 whenever crust lands
    /// outside it, and nothing ever shrinks it back, so a plate's box drifts far
    /// past its contents. On a 2048x2048 run the plates' boxes reach 4.7x the
    /// world area while holding 1x the world area of crust, and
    /// `update_height_and_plate_index_maps` scans every cell of every box.
    ///
    /// Must be called before `reset_segments`: the segment ids are dropped here
    /// rather than remapped, since they are about to be rebuilt anyway.
    ///
    /// `margin` empty cells are kept around the crust, so that crust cells stay
    /// as interior as they were and the erosion boundary condition is unchanged.
    ///
    /// Returns whether the bounds changed.
    pub fn compact_bounds(&mut self, margin: u32) -> bool {
        let w = self.bounds.width();
        let h = self.bounds.height();

        // Tight box of the non-empty crust, in plate-local coordinates. The
        // threshold matches the one `update_height_and_plate_index_maps` uses to
        // decide a cell holds crust.
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (w, h, 0u32, 0u32);
        let mut k = 0usize;
        for y in 0..h {
            for x in 0..w {
                if self.map[k] >= 2.0 * f32::EPSILON {
                    if x < min_x {
                        min_x = x;
                    }
                    if x > max_x {
                        max_x = x;
                    }
                    if y < min_y {
                        min_y = y;
                    }
                    if y > max_y {
                        max_y = y;
                    }
                }
                k += 1;
            }
        }

        // No crust at all: leave it to `remove_empty_plates`.
        if min_x > max_x {
            return false;
        }

        // Keep a margin of empty cells around the crust. `erode` treats cells on
        // the box edge differently from interior ones, so shrinking flush to the
        // crust would quietly change the erosion boundary condition; the margin
        // keeps every crust cell as interior as it was before.
        min_x = min_x.saturating_sub(margin);
        min_y = min_y.saturating_sub(margin);
        max_x = (max_x + margin).min(w - 1);
        max_y = (max_y + margin).min(h - 1);

        if min_x == 0 && min_y == 0 && max_x == w - 1 && max_y == h - 1 {
            return false;
        }

        let (new_w, new_h) = (max_x - min_x + 1, max_y - min_y + 1);
        let mut tmph = HeightMap::new(new_w, new_h);
        let mut tmpa = AgeMap::new(new_w, new_h);
        for j in 0..new_h {
            let src = ((min_y + j) * w + min_x) as usize;
            let dst = (j * new_w) as usize;
            let n = new_w as usize;
            tmph.as_mut_slice()[dst..dst + n]
                .copy_from_slice(&self.map.as_slice()[src..src + n]);
            tmpa.as_mut_slice()[dst..dst + n]
                .copy_from_slice(&self.age_map.as_slice()[src..src + n]);
        }
        self.map = tmph;
        self.age_map = tmpa;
        self.bounds.compact(min_x, min_y, new_w, new_h);

        let area = self.bounds.area();
        self.segments.reassign(area, vec![u32::MAX; area as usize]);

        // The centre of mass is in plate-local coordinates, so moving the origin
        // moves it too. Translate it rather than rebuilding it with a
        // `MassBuilder`: between erosions the stored centre is deliberately
        // stale (only `erode` recomputes it), and refitting the box must not
        // quietly refresh it — that changes collision dynamics and measurably
        // flattens the world. The centre always lies inside the crust box, so
        // the subtraction cannot go negative.
        self.mass = Mass::new(
            self.mass.get_mass(),
            self.mass.get_cx() - min_x as f32,
            self.mass.get_cy() - min_y as f32,
        );
        true
    }

    pub fn reset_segments(&mut self) {
        platec_assert!(
            self.bounds.area() == self.segments.area(),
            "Segments doesn't have the expected area"
        );
        self.segments.reset();
    }

    /// Remember the currently processed continent's segment number.
    pub fn select_collision_segment(&self, coll_x: u32, coll_y: u32) -> ContinentId {
        let mut lx = coll_x;
        let mut ly = coll_y;
        let index = self.bounds.get_valid_map_index(&mut lx, &mut ly);
        self.segments.id(index)
    }

    /// Set the amount of the plate's crustal material at some location. If the
    /// amount is negative it is set to zero.
    pub fn set_crust(&mut self, x: u32, y: u32, mut z: f32, mut t: u32) {
        if z < 0.0 {
            // Do not accept negative values.
            z = 0.0;
        }

        let mut lx = x;
        let mut ly = y;
        let mut index = self.bounds.get_map_index(&mut lx, &mut ly);

        if index == BAD_INDEX {
            // Extending the plate for nothing!
            platec_assert!(z > 0.0, "Height value must be non-zero");

            let ilft = self.bounds.left_as_uint();
            let itop = self.bounds.top_as_uint();
            let irgt = self.bounds.right_as_uint_non_inclusive();
            let ibtm = self.bounds.bottom_as_uint_non_inclusive();

            let mut x = x;
            let mut y = y;
            self.world_dimension.normalize(&mut x, &mut y);

            let world_w = self.world_dimension.get_width();
            let world_h = self.world_dimension.get_height();

            // Calculate the distance of the new point from the plate edges.
            // Every one of these is a deliberate unsigned wraparound.
            let n_lft = ilft.wrapping_sub(x);
            let n_rgt = (world_w & u32::from(x < ilft).wrapping_neg())
                .wrapping_add(x)
                .wrapping_sub(irgt);
            let n_top = itop.wrapping_sub(y);
            let n_btm = (world_h & u32::from(y < itop).wrapping_neg())
                .wrapping_add(y)
                .wrapping_sub(ibtm);

            // Set the larger of the horizontal/vertical distance to zero.
            // A valid distance is NEVER larger than the world's side length!
            let mut d_lft = n_lft
                & u32::from(n_lft < n_rgt).wrapping_neg()
                & u32::from(n_lft < world_w).wrapping_neg();
            let mut d_rgt = n_rgt
                & u32::from(n_rgt <= n_lft).wrapping_neg()
                & u32::from(n_rgt < world_w).wrapping_neg();
            let mut d_top = n_top
                & u32::from(n_top < n_btm).wrapping_neg()
                & u32::from(n_top < world_h).wrapping_neg();
            let mut d_btm = n_btm
                & u32::from(n_btm <= n_top).wrapping_neg()
                & u32::from(n_btm < world_h).wrapping_neg();

            // Scale all changes to a multiple of 8.
            d_lft = (u32::from(d_lft > 0).wrapping_add(d_lft >> 3)) << 3;
            d_rgt = (u32::from(d_rgt > 0).wrapping_add(d_rgt >> 3)) << 3;
            d_top = (u32::from(d_top > 0).wrapping_add(d_top >> 3)) << 3;
            d_btm = (u32::from(d_btm > 0).wrapping_add(d_btm >> 3)) << 3;

            // Make sure the plate doesn't grow bigger than the system it's in!
            if self.bounds.width() + d_lft + d_rgt > world_w {
                d_lft = 0;
                d_rgt = world_w - self.bounds.width();
            }

            if self.bounds.height() + d_top + d_btm > world_h {
                d_top = 0;
                d_btm = world_h - self.bounds.height();
            }

            // Index out of bounds, but nowhere to grow!
            platec_assert!(
                d_lft + d_rgt + d_top + d_btm != 0,
                "Invalid plate growth deltas"
            );

            let old_width = self.bounds.width();
            let old_height = self.bounds.height();

            self.bounds.shift(-1.0 * d_lft as f32, -1.0 * d_top as f32);
            self.bounds.grow((d_lft + d_rgt) as i32, (d_top + d_btm) as i32);

            let new_width = self.bounds.width();
            let new_area = self.bounds.area();
            let mut tmph = HeightMap::new(new_width, self.bounds.height());
            let mut tmpa = AgeMap::new(new_width, self.bounds.height());
            let mut tmps = vec![u32::MAX; new_area as usize];
            tmph.set_all(0.0);
            tmpa.set_all(0);

            // Copy the old plate into the new one.
            for j in 0..old_height {
                let dest_i = ((d_top + j) * new_width + d_lft) as usize;
                let src_i = (j * old_width) as usize;
                let w = old_width as usize;
                tmph.as_mut_slice()[dest_i..dest_i + w]
                    .copy_from_slice(&self.map.as_slice()[src_i..src_i + w]);
                tmpa.as_mut_slice()[dest_i..dest_i + w]
                    .copy_from_slice(&self.age_map.as_slice()[src_i..src_i + w]);
                for k in 0..w {
                    tmps[dest_i + k] = self.segments.id((src_i + k) as u32);
                }
            }

            self.map = tmph;
            self.age_map = tmpa;
            self.segments.reassign(new_area, tmps);

            // Shift all segment data to match the new coordinates.
            self.segments.shift(d_lft, d_top);

            lx = x;
            ly = y;
            index = self.bounds.get_valid_map_index(&mut lx, &mut ly);

            debug_assert!(index < self.bounds.area());
        }

        let index = index as usize;

        // Update the crust's age. If old crust exists, the new age is the mean
        // of the original and supplied ages. If no new crust is added, the
        // original time remains intact.
        let old_crust = u32::from(self.map[index] > 0.0).wrapping_neg();
        let new_crust = u32::from(z > 0.0).wrapping_neg();
        t = (t & !old_crust)
            | (((self.map[index] * self.age_map[index] as f32 + z * t as f32)
                / (self.map[index] + z)) as u32
                & old_crust);
        self.age_map[index] = (t & new_crust) | (self.age_map[index] & !new_crust);

        // Clamp to prevent floating point precision errors (Issue #30).
        if z < 0.0 {
            z = 0.0;
        }

        self.mass.inc_mass(-1.0 * self.map[index]);
        self.mass.inc_mass(z); // Update the mass counter.
        self.map[index] = z; // Set the new crust height at the desired location.
    }

    // ---------------------------------------------------------------------
    // Accessors
    // ---------------------------------------------------------------------

    pub fn get_momentum(&self) -> f32 {
        self.movement.momentum(&self.mass)
    }
    pub fn get_height(&self) -> u32 {
        self.bounds.height()
    }
    pub fn get_width(&self) -> u32 {
        self.bounds.width()
    }
    pub fn get_left_as_uint(&self) -> u32 {
        self.bounds.left_as_uint()
    }
    pub fn get_top_as_uint(&self) -> u32 {
        self.bounds.top_as_uint()
    }
    pub fn get_velocity(&self) -> f32 {
        self.movement.get_velocity()
    }
    /// Deprecated; use [`MovementLike::velocity_unit_vector`].
    pub fn get_vel_x(&self) -> f32 {
        self.movement.vel_x()
    }
    /// Deprecated; use [`MovementLike::velocity_unit_vector`].
    pub fn get_vel_y(&self) -> f32 {
        self.movement.vel_y()
    }
    pub fn is_empty(&self) -> bool {
        self.mass.null()
    }
    pub fn get_cx(&self) -> f32 {
        self.mass.get_cx()
    }
    pub fn get_cy(&self) -> f32 {
        self.mass.get_cy()
    }
    pub fn dec_dx(&mut self, delta: f32) {
        self.movement.dec_dx(delta);
    }
    pub fn dec_dy(&mut self, delta: f32) {
        self.movement.dec_dy(delta);
    }

    /// Mutable access to the age map, needed by `lithosphere::restart` (which
    /// needs to mutate the segments).
    #[allow(dead_code)]
    pub(crate) fn age_map_mut(&mut self) -> &mut AgeMap {
        &mut self.age_map
    }

    /// Visible for testing.
    #[doc(hidden)]
    pub fn inject_segments(&mut self, segments: Box<dyn SegmentsApi + Send>) {
        self.segments = segments;
    }

    /// The continent ID at a world location, creating the segment lazily.
    ///
    /// The lazy creation lives inside `Segments::get_continent_at`,
    /// which reaches back into the plate through raw pointers. Here the pieces
    /// it needs are handed over explicitly.
    fn continent_id_at(&mut self, x: u32, y: u32) -> ContinentId {
        let ctx = SegmentCtx {
            bounds: &self.bounds,
            map: &self.map,
            world_dimension: &self.world_dimension,
        };
        self.segments.get_continent_at(x, y, &ctx)
    }
}

impl MassLike for Plate {
    fn get_mass(&self) -> f32 {
        self.mass.get_mass()
    }
    fn mass_center(&self) -> FloatPoint {
        self.mass.mass_center()
    }
}

impl MovementLike for Plate {
    fn velocity_unit_vector(&self) -> FloatVector {
        self.movement.velocity_unit_vector()
    }
    fn dec_impulse(&mut self, delta: FloatVector) {
        self.movement.dec_dx(delta.x());
        self.movement.dec_dy(delta.y());
    }
}
