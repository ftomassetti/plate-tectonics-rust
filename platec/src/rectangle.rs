//! Port of `src/rectangle.hpp` / `src/rectangle.cpp`.

use crate::geometry::WorldDimension;
use crate::platec_assert;

/// Port of the C++ `#define BAD_INDEX 0xFFFFFFFF`.
pub const BAD_INDEX: u32 = 0xFFFF_FFFF;

/// Port of `Platec::Rectangle`.
///
/// The C++ stores `const WorldDimension _worldDimension` by value; we do the
/// same (it is `Copy`).
#[derive(Clone, Copy, Debug)]
pub struct Rectangle {
    world_dimension: WorldDimension,
    left: u32,
    right: u32,
    top: u32,
    bottom: u32,
}

impl Rectangle {
    pub fn new(
        world_dimension: WorldDimension,
        left: u32,
        right: u32,
        top: u32,
        bottom: u32,
    ) -> Self {
        Self {
            world_dimension,
            left,
            right,
            top,
            bottom,
        }
    }

    /// Return a valid index or [`BAD_INDEX`].
    ///
    /// On success `px`/`py` are rewritten to the local (plate-relative)
    /// coordinates. Ported verbatim, including the unsigned wraparound: the
    /// comment in the original reads "If you think you're smart enough to
    /// optimize this then PREPARE to be smart as HELL to debug it!".
    pub fn get_map_index(&self, px: &mut u32, py: &mut u32) -> u32 {
        let world_width = self.world_dimension.get_width();
        let world_height = self.world_dimension.get_height();
        let mut x = *px % world_width;
        let mut y = *py % world_height;

        let ilft = self.left;
        let itop = self.top;
        let irgt = self
            .right
            .wrapping_add(if self.right < ilft { world_width } else { 0 });
        let ibtm = self
            .bottom
            .wrapping_add(if self.bottom < itop { world_height } else { 0 });
        let width = irgt.wrapping_sub(ilft) as i32;
        platec_assert!(width >= 0, "Width must be postive");

        let x_plus_width = x.wrapping_add(world_width);
        let x_ok_a = x >= ilft && x < irgt;
        let x_ok_b = x_plus_width >= ilft && x_plus_width < irgt;
        let x_ok = x_ok_a || x_ok_b;

        let y_plus_height = y.wrapping_add(world_height);
        let y_ok_a = y >= itop && y < ibtm;
        let y_ok_b = y_plus_height >= itop && y_plus_height < ibtm;
        let y_ok = y_ok_a || y_ok_b;

        // Point is within plate's map: wrap it around world edges if necessary.
        x = x.wrapping_add(if x < ilft { world_width } else { 0 });
        y = y.wrapping_add(if y < itop { world_height } else { 0 });

        platec_assert!(x >= ilft && y >= itop, "Coordinates must be positive");
        x = x.wrapping_sub(ilft); // Calculate offset within local map.
        y = y.wrapping_sub(itop);

        if x_ok && y_ok {
            *px = x;
            *py = y;
            y.wrapping_mul(width as u32).wrapping_add(x)
        } else {
            BAD_INDEX
        }
    }

    pub fn enlarge_to_contain(&mut self, x: u32, y: u32) {
        if y < self.top {
            self.top = y;
        } else if y > self.bottom {
            self.bottom = y;
        }
        if x < self.left {
            self.left = x;
        } else if x > self.right {
            self.right = x;
        }
    }

    pub fn get_left(&self) -> u32 {
        self.left
    }
    pub fn get_right(&self) -> u32 {
        self.right
    }
    pub fn get_top(&self) -> u32 {
        self.top
    }
    pub fn get_bottom(&self) -> u32 {
        self.bottom
    }
    pub fn set_left(&mut self, v: u32) {
        self.left = v;
    }
    pub fn set_right(&mut self, v: u32) {
        self.right = v;
    }
    pub fn set_top(&mut self, v: u32) {
        self.top = v;
    }
    pub fn set_bottom(&mut self, v: u32) {
        self.bottom = v;
    }

    /// `dx`/`dy` are frequently huge unsigned values standing in for negative
    /// deltas (see `Segments::shift`), so the additions must wrap.
    pub fn shift(&mut self, dx: u32, dy: u32) {
        self.left = self.left.wrapping_add(dx);
        self.right = self.right.wrapping_add(dx);
        self.top = self.top.wrapping_add(dy);
        self.bottom = self.bottom.wrapping_add(dy);
    }

    pub fn contains(&self, x: u32, y: u32) -> bool {
        let mut clean_x = self.world_dimension.x_mod(x);
        let mut clean_y = self.world_dimension.y_mod(y);
        if clean_x < self.get_left() {
            clean_x = clean_x.wrapping_add(self.world_dimension.get_width());
        }
        if clean_y < self.get_top() {
            clean_y = clean_y.wrapping_add(self.world_dimension.get_height());
        }
        clean_x >= self.get_left()
            && clean_x < self.get_right()
            && clean_y >= self.get_top()
            && clean_y < self.get_bottom()
    }
}
