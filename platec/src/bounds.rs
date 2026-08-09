//! Port of `src/bounds.hpp` / `src/bounds.cpp`.
//!
//! The C++ `IBounds` interface exists only so that `plate` can have a bounds
//! object injected; no test actually does so and `Bounds` is the sole
//! implementor, so the trait is dropped here in favour of the concrete struct.

use crate::geometry::{Dimension, FloatPoint, WorldDimension};
use crate::platec_assert;
use crate::rectangle::{Rectangle, BAD_INDEX};

/// Plate bounds.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    world_dimension: WorldDimension,
    position: FloatPoint,
    dimension: Dimension,
}

impl Bounds {
    /// * `world_dimension` — dimension of the world containing the plate
    /// * `position` — position of the top left corner of the plate
    /// * `dimension` — dimension of the plate
    pub fn new(
        world_dimension: WorldDimension,
        position: FloatPoint,
        dimension: Dimension,
    ) -> Self {
        platec_assert!(
            dimension.get_width() <= world_dimension.get_width()
                && dimension.get_height() <= world_dimension.get_height(),
            "Bounds are larger than the world containing it"
        );
        Self {
            world_dimension,
            position,
            dimension,
        }
    }

    /// Accept plate relative coordinates and return the index inside the plate.
    pub fn index(&self, x: u32, y: u32) -> u32 {
        platec_assert!(
            x < self.dimension.get_width() && y < self.dimension.get_height(),
            "Invalid coordinates"
        );
        y.wrapping_mul(self.dimension.get_width()).wrapping_add(x)
    }

    /// Total area occupied by the plate (width * height).
    pub fn area(&self) -> u32 {
        self.dimension.get_area()
    }

    pub fn width(&self) -> u32 {
        self.dimension.get_width()
    }

    pub fn height(&self) -> u32 {
        self.dimension.get_height()
    }

    /// Left position of the plate in world coordinates.
    pub fn left_as_uint(&self) -> u32 {
        self.position.get_x() as u32
    }

    /// Top position of the plate in world coordinates.
    pub fn top_as_uint(&self) -> u32 {
        self.position.get_y() as u32
    }

    /// Despite the name, the C++ computes `left + width - 1`, i.e. the last
    /// point that *is* part of the plate. Preserved verbatim — the tests pin it.
    pub fn right_as_uint_non_inclusive(&self) -> u32 {
        self.left_as_uint()
            .wrapping_add(self.width())
            .wrapping_sub(1)
    }

    pub fn bottom_as_uint_non_inclusive(&self) -> u32 {
        self.top_as_uint()
            .wrapping_add(self.height())
            .wrapping_sub(1)
    }

    /// Given a point in world relative coordinates, tell whether it is part of
    /// the plate.
    pub fn contains_world_point(&self, x: u32, y: u32) -> bool {
        self.as_rect().contains(x, y)
    }

    /// Given a point in plate relative coordinates, tell whether it is part of
    /// the plate.
    pub fn is_in_limits(&self, x: f32, y: f32) -> bool {
        if x < 0.0 {
            return false;
        }
        if y < 0.0 {
            return false;
        }
        let ux = x as u32;
        let uy = y as u32;
        ux < self.dimension.get_width() && uy < self.dimension.get_height()
    }

    /// Shift the top-left corner by the given amount, preserving dimensions.
    pub fn shift(&mut self, dx: f32, dy: f32) {
        let world = self.world_dimension;
        self.position.shift(dx, dy, &world);
        platec_assert!(world.contains_point(&self.position), "Point not in world!");
    }

    /// Grow the plate towards the right and the bottom.
    /// `dx`/`dy` must be positive or zero.
    pub fn grow(&mut self, dx: i32, dy: i32) {
        platec_assert!(dx >= 0 && dy >= 0, "Negative delta is not allowed");
        self.dimension.grow(dx as u32, dy as u32);

        platec_assert!(
            self.dimension.get_width() <= self.world_dimension.get_width(),
            "Bounds are larger than the world containing it"
        );
        platec_assert!(
            self.dimension.get_height() <= self.world_dimension.get_height(),
            format!(
                "Bounds taller than the world containing it. delta={} resulting plate height={} world height={}",
                dy,
                self.dimension.get_height(),
                self.world_dimension.get_height()
            )
        );
    }

    /// Return a rectangle representing the bounds inside the world.
    fn as_rect(&self) -> Rectangle {
        let ilft = self.left_as_uint();
        let itop = self.top_as_uint();
        let irgt = ilft.wrapping_add(self.dimension.get_width());
        let ibtm = itop.wrapping_add(self.dimension.get_height());
        Rectangle::new(self.world_dimension, ilft, irgt, itop, ibtm)
    }

    /// Translate world coordinates into an offset within the plate's height
    /// map. On failure the coordinates are left intact and [`BAD_INDEX`] is
    /// returned.
    pub fn get_map_index(&self, px: &mut u32, py: &mut u32) -> u32 {
        self.as_rect().get_map_index(px, py)
    }

    /// As [`Bounds::get_map_index`], but asserts that the index is valid.
    pub fn get_valid_map_index(&self, px: &mut u32, py: &mut u32) -> u32 {
        let res = self.as_rect().get_map_index(px, py);
        platec_assert!(res != BAD_INDEX, "BAD map index found");
        res
    }

    pub fn world_dimension(&self) -> WorldDimension {
        self.world_dimension
    }

    pub fn position(&self) -> FloatPoint {
        self.position
    }

    pub fn dimension(&self) -> Dimension {
        self.dimension
    }
}
