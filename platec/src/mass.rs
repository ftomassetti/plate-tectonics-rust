//! Plate mass and centre of mass.

use crate::geometry::{Dimension, FloatPoint};
use crate::platec_assert;

/// `Movement::collide` takes one of these, and `test_movement` supplies a mock,
/// so it stays a trait.
pub trait MassLike {
    fn get_mass(&self) -> f32;
    fn mass_center(&self) -> FloatPoint;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MassBuilder {
    /// Amount of crust that constitutes the plate.
    mass: f32,
    /// X and Y components of the centre of mass of the plate.
    cx: f32,
    cy: f32,
}

impl MassBuilder {
    pub fn new() -> Self {
        Self {
            mass: 0.0,
            cx: 0.0,
            cy: 0.0,
        }
    }

    pub fn from_slice(m: &[f32], dimension: &Dimension) -> Self {
        let mut builder = MassBuilder::new();
        let mut k = 0usize;
        for y in 0..dimension.get_height() {
            for x in 0..dimension.get_width() {
                platec_assert!(m[k] >= 0.0, "Crust should be not negative");
                builder.add_point(x, y, m[k]);
                k += 1;
            }
        }
        builder
    }

    pub fn add_point(&mut self, x: u32, y: u32, crust: f32) {
        platec_assert!(crust >= 0.0, "Crust should be not negative");
        self.mass += crust;
        // Update the center coordinates weighted by mass.
        self.cx += x as f32 * crust;
        self.cy += y as f32 * crust;
    }

    pub fn build(&self) -> Mass {
        if self.mass <= 0.0 {
            Mass::new(0.0, 0.0, 0.0)
        } else {
            let inv_mass = 1.0 / self.mass;
            Mass::new(self.mass, self.cx * inv_mass, self.cy * inv_mass)
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Mass {
    /// Amount of crust that constitutes the plate.
    mass: f32,
    /// X and Y components of the centre of mass of the plate.
    cx: f32,
    cy: f32,
}

impl Mass {
    pub fn new(mass: f32, cx: f32, cy: f32) -> Self {
        Self { mass, cx, cy }
    }

    pub fn inc_mass(&mut self, delta: f32) {
        self.mass += delta;
        // Clamp negative mass to zero to handle floating point precision errors
        // that accumulate over many iterations (Issue #30).
        if self.mass < 0.0 {
            self.mass = 0.0;
        }
    }

    pub fn get_cx(&self) -> f32 {
        self.cx
    }

    pub fn get_cy(&self) -> f32 {
        self.cy
    }

    pub fn null(&self) -> bool {
        self.mass <= 0.0
    }
}

impl MassLike for Mass {
    fn get_mass(&self) -> f32 {
        self.mass
    }

    fn mass_center(&self) -> FloatPoint {
        FloatPoint::new(self.cx, self.cy)
    }
}
