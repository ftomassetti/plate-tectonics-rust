//! A point on the world map.

use crate::geometry::WorldDimension;
use crate::platec_assert;

/// Immutable point expressed in World coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldPoint {
    x: u32,
    y: u32,
}

impl WorldPoint {
    pub fn new(x: u32, y: u32, dim: &WorldDimension) -> Self {
        platec_assert!(
            x < dim.get_width() && y < dim.get_height(),
            "Point outside of world!"
        );
        Self { x, y }
    }

    pub fn x(&self) -> u32 {
        self.x
    }

    pub fn y(&self) -> u32 {
        self.y
    }

    pub fn to_index(&self, dim: &WorldDimension) -> u32 {
        platec_assert!(
            self.x < dim.get_width() && self.y < dim.get_height(),
            "Point outside of world!"
        );
        self.y.wrapping_mul(dim.get_width()).wrapping_add(self.x)
    }
}
