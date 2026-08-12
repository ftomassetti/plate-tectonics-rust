//! Vectors, points and dimensions.

use crate::platec_assert;
use std::ops::{Mul, Sub};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntVector {
    x: i32,
    y: i32,
}

impl IntVector {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
    pub fn x(&self) -> i32 {
        self.x
    }
    pub fn y(&self) -> i32 {
        self.y
    }
    pub fn length(&self) -> f32 {
        // The product is computed in integer arithmetic, then widened.
        ((self.x.wrapping_mul(self.x).wrapping_add(self.y.wrapping_mul(self.y))) as f32).sqrt()
    }
}

impl Sub for IntVector {
    type Output = IntVector;
    fn sub(self, other: IntVector) -> IntVector {
        IntVector::new(self.x - other.x, self.y - other.y)
    }
}

/// A point with integer coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntPoint {
    x: i32,
    y: i32,
}

impl IntPoint {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
    pub fn get_x(&self) -> i32 {
        self.x
    }
    pub fn get_y(&self) -> i32 {
        self.y
    }
}

impl Sub for IntPoint {
    type Output = IntVector;
    fn sub(self, other: IntPoint) -> IntVector {
        IntVector::new(self.x - other.x, self.y - other.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloatVector {
    x: f32,
    y: f32,
}

impl FloatVector {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub fn x(&self) -> f32 {
        self.x
    }
    pub fn y(&self) -> f32 {
        self.y
    }
    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    /// Normalize in place, returning the original length.
    pub fn normalize(&mut self) -> f32 {
        let len = self.length();
        if len > 0.0 {
            let inv_len = 1.0 / len;
            self.x *= inv_len;
            self.y *= inv_len;
        }
        len
    }
    pub fn to_int_vector(&self) -> IntVector {
        IntVector::new(self.x as i32, self.y as i32)
    }
    pub fn dot_product(&self, other: &FloatVector) -> f32 {
        self.x * other.x + self.y * other.y
    }
}

impl Sub for FloatVector {
    type Output = FloatVector;
    fn sub(self, other: FloatVector) -> FloatVector {
        FloatVector::new(self.x - other.x, self.y - other.y)
    }
}

impl Mul<f32> for FloatVector {
    type Output = FloatVector;
    fn mul(self, f: f32) -> FloatVector {
        FloatVector::new(self.x * f, self.y * f)
    }
}

/// A point with float coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloatPoint {
    x: f32,
    y: f32,
}

impl FloatPoint {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub fn get_x(&self) -> f32 {
        self.x
    }
    pub fn get_y(&self) -> f32 {
        self.y
    }

    /// Move a point by the given delta, wrapping around the borders of the
    /// world if needed.
    ///
    /// Note the exact conditions: `> 0` (not `>= 0`) and `< world_width`, ported
    /// deliberate.
    pub fn shift(&mut self, dx: f32, dy: f32, world_dimension: &WorldDimension) {
        let world_width = world_dimension.get_width() as f32;
        self.x += dx;
        self.x += if self.x > 0.0 { 0.0 } else { world_width };
        self.x -= if self.x < world_width { 0.0 } else { world_width };

        let world_height = world_dimension.get_height() as f32;
        self.y += dy;
        self.y += if self.y > 0.0 { 0.0 } else { world_height };
        self.y -= if self.y < world_height { 0.0 } else { world_height };

        platec_assert!(world_dimension.contains_point(self), "Point not in world!");
    }

    /// Translate to an `IntPoint` (using the truncate operation).
    pub fn to_int(&self) -> IntPoint {
        IntPoint::new(self.x as i32, self.y as i32)
    }
}

impl Sub for FloatPoint {
    type Output = FloatVector;
    fn sub(self, other: FloatPoint) -> FloatVector {
        FloatVector::new(self.x - other.x, self.y - other.y)
    }
}

/// Dimension of a rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dimension {
    width: u32,
    height: u32,
}

impl Dimension {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
    pub fn get_width(&self) -> u32 {
        self.width
    }
    pub fn get_height(&self) -> u32 {
        self.height
    }
    pub fn get_area(&self) -> u32 {
        self.width.wrapping_mul(self.height)
    }
    pub fn contains(&self, x: u32, y: u32) -> bool {
        x < self.width && y < self.height
    }
    pub fn contains_f(&self, x: f32, y: f32) -> bool {
        x >= 0.0 && x < self.width as f32 && y >= 0.0 && y < self.height as f32
    }
    pub fn contains_point(&self, p: &FloatPoint) -> bool {
        self.contains_f(p.get_x(), p.get_y())
    }
    pub fn grow(&mut self, amount_x: u32, amount_y: u32) {
        self.width = self.width.wrapping_add(amount_x);
        self.height = self.height.wrapping_add(amount_y);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldDimension {
    dim: Dimension,
}

impl WorldDimension {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            dim: Dimension::new(width, height),
        }
    }

    // --- inherited from Dimension ---
    pub fn get_width(&self) -> u32 {
        self.dim.get_width()
    }
    pub fn get_height(&self) -> u32 {
        self.dim.get_height()
    }
    pub fn get_area(&self) -> u32 {
        self.dim.get_area()
    }
    pub fn contains(&self, x: u32, y: u32) -> bool {
        self.dim.contains(x, y)
    }
    pub fn contains_f(&self, x: f32, y: f32) -> bool {
        self.dim.contains_f(x, y)
    }
    pub fn contains_point(&self, p: &FloatPoint) -> bool {
        self.dim.contains_point(p)
    }
    pub fn grow(&mut self, amount_x: u32, amount_y: u32) {
        self.dim.grow(amount_x, amount_y)
    }
    pub fn as_dimension(&self) -> Dimension {
        self.dim
    }

    // --- WorldDimension proper ---
    pub fn get_max(&self) -> u32 {
        if self.get_width() > self.get_height() {
            self.get_width()
        } else {
            self.get_height()
        }
    }
    pub fn x_mod(&self, x: u32) -> u32 {
        x.wrapping_add(self.get_width()) % self.get_width()
    }
    pub fn y_mod(&self, y: u32) -> u32 {
        y.wrapping_add(self.get_height()) % self.get_height()
    }
    pub fn normalize(&self, x: &mut u32, y: &mut u32) {
        *x %= self.get_width();
        *y %= self.get_height();
    }
    pub fn index_of(&self, x: u32, y: u32) -> u32 {
        y.wrapping_mul(self.get_width()).wrapping_add(x)
    }
    pub fn line_index(&self, y: u32) -> u32 {
        platec_assert!(y < self.get_height(), "y is not valid");
        self.index_of(0, y)
    }
    pub fn y_from_index(&self, index: u32) -> u32 {
        index / self.get_width()
    }
    pub fn x_from_index(&self, index: u32) -> u32 {
        let y = self.y_from_index(index);
        index.wrapping_sub(y.wrapping_mul(self.get_width()))
    }
    pub fn normalized_index_of(&self, x: u32, y: u32) -> u32 {
        self.index_of(self.x_mod(x), self.y_mod(y))
    }
    pub fn x_cap(&self, x: u32) -> u32 {
        if x < self.get_width() {
            x
        } else {
            self.get_width() - 1
        }
    }
    pub fn y_cap(&self, y: u32) -> u32 {
        if y < self.get_height() {
            y
        } else {
            self.get_height() - 1
        }
    }
    pub fn larger_size(&self) -> u32 {
        self.get_max()
    }
}
