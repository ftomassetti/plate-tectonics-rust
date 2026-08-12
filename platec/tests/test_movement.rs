//! Tests for plate velocity, rotation and collision response.

// Several lints fire on constructs kept deliberately for numerical fidelity
// (literal precision, `-1.0 * x`, negated float comparisons, and so on).
#![allow(clippy::excessive_precision)]
#![allow(clippy::manual_abs_diff)]
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::neg_cmp_op_on_partial_ord)]
#![allow(clippy::neg_multiply)]
#![allow(clippy::should_implement_trait)]

#[macro_use]
mod common;

use platec::geometry::{FloatPoint, FloatVector, WorldDimension};
use platec::mass::{Mass, MassLike};
use platec::movement::{Movement, MovementLike};
use platec::simplerandom::SimpleRandom;

#[test]
fn movement_constructor() {
    let sr = SimpleRandom::new(123);
    let wd = WorldDimension::new(5, 4);
    let mov = Movement::new(sr, wd);
    expect_float_eq!(0.99992257f32, mov.vel_x());
    expect_float_eq!(0.01244594f32, mov.vel_y());
    expect_float_eq!(1.0f32, mov.get_velocity());
}

#[test]
fn movement_apply_friction() {
    let sr = SimpleRandom::new(456);
    let wd = WorldDimension::new(50, 40);
    let mut mov = Movement::new(sr, wd);

    expect_float_eq!(0.9989379f32, mov.vel_x());
    expect_float_eq!(0.046077024f32, mov.vel_y());

    mov.apply_friction(2.2, 10.5);
    expect_float_eq!(0.9989379f32, mov.vel_x());
    expect_float_eq!(0.046077024f32, mov.vel_y());
    expect_float_eq!(0.58095241f32, mov.get_velocity());

    mov.apply_friction(7.2, 0.0);
    expect_float_eq!(0.0f32, mov.get_velocity());
}

#[test]
fn movement_apply_friction_with_null_mass() {
    let sr = SimpleRandom::new(456);
    let wd = WorldDimension::new(50, 40);
    let mut mov = Movement::new(sr, wd);

    mov.apply_friction(7.2, 0.0);
    expect_float_eq!(0.0f32, mov.get_velocity());
}

#[test]
fn movement_move() {
    let sr = SimpleRandom::new(789890);
    let wd = WorldDimension::new(500, 400);
    let mut mov = Movement::new(sr, wd);

    expect_float_eq!(-0.29389676f32, mov.vel_x());
    expect_float_eq!(-0.95583719f32, mov.vel_y());
    expect_float_eq!(1.0f32, mov.get_velocity());

    mov.move_plate();
    expect_float_eq!(-0.28745356f32, mov.vel_x());
    expect_float_eq!(-0.95779467f32, mov.vel_y());
    expect_float_eq!(1.0f32, mov.get_velocity());
}

#[test]
fn movement_velocity_on_x_no_params() {
    let mov = Movement::new(SimpleRandom::new(789890), WorldDimension::new(500, 400));
    expect_float_eq!(-0.29389676f32, mov.vel_x());
    expect_float_eq!(1.0f32, mov.get_velocity());
    expect_float_eq!(-0.29389676f32, mov.velocity_on_x());
}

#[test]
fn movement_velocity_on_y_no_params() {
    let mov = Movement::new(SimpleRandom::new(789890), WorldDimension::new(500, 400));
    expect_float_eq!(-0.95583719f32, mov.vel_y());
    expect_float_eq!(1.0f32, mov.get_velocity());
    expect_float_eq!(-0.95583719f32, mov.velocity_on_y());
}

#[test]
fn movement_velocity_on_x_one_param() {
    let mov = Movement::new(SimpleRandom::new(789890), WorldDimension::new(500, 400));
    expect_float_eq!(-0.29389676f32, mov.vel_x());
    expect_float_eq!(1.0f32, mov.get_velocity());
    expect_float_eq!(-2.9389676f32, mov.velocity_on_x_len(10.0));
}

#[test]
fn movement_velocity_on_y_one_param() {
    let mov = Movement::new(SimpleRandom::new(789890), WorldDimension::new(500, 400));
    expect_float_eq!(-0.95583719f32, mov.vel_y());
    expect_float_eq!(1.0f32, mov.get_velocity());
    expect_float_eq!(-9.5583719f32, mov.velocity_on_y_len(10.0));
}

#[test]
fn movement_dot() {
    let mov = Movement::new(SimpleRandom::new(789890), WorldDimension::new(500, 400));
    expect_float_eq!(-0.29389676f32, mov.vel_x());
    expect_float_eq!(-0.95583719f32, mov.vel_y());
    expect_float_eq!(-3.45530509f32, mov.dot(2.0, 3.0));
}

/// A stand-in plate: mass plus motion.
struct MockPlate {
    velocity_unit_vector: FloatVector,
    dec_impulse_delta: Option<FloatVector>,
    mass: f32,
    mass_center: FloatPoint,
}

impl MockPlate {
    fn new(velocity_unit_vector: FloatVector, mass: f32, mass_center: FloatPoint) -> Self {
        Self {
            velocity_unit_vector,
            dec_impulse_delta: None,
            mass,
            mass_center,
        }
    }

    fn dec_impulse_delta(&self) -> FloatVector {
        self.dec_impulse_delta
            .expect("(MockPlate::decImpulseDelta) Data not ready")
    }
}

impl MassLike for MockPlate {
    fn get_mass(&self) -> f32 {
        self.mass
    }
    fn mass_center(&self) -> FloatPoint {
        self.mass_center
    }
}

impl MovementLike for MockPlate {
    fn velocity_unit_vector(&self) -> FloatVector {
        self.velocity_unit_vector
    }
    fn dec_impulse(&mut self, delta: FloatVector) {
        self.dec_impulse_delta = Some(delta);
    }
}

#[test]
fn movement_collide() {
    let sr = SimpleRandom::new(789890);
    let wd = WorldDimension::new(500, 400);
    let mut mov = Movement::new(sr, wd);

    expect_float_eq!(-0.29389676f32, mov.vel_x());
    expect_float_eq!(-0.95583719f32, mov.vel_y());
    expect_float_eq!(1.0f32, mov.get_velocity());

    let this_mass = Mass::new(100.0, 70.0, 90.0);
    let other_plate_velocity_unit_vector = FloatVector::new(0.0, -1.0);
    let other_plate_mass = 10000.0f32;
    let other_plate_mass_center = FloatPoint::new(100.0, 400.0);
    let mut other_plate = MockPlate::new(
        other_plate_velocity_unit_vector,
        other_plate_mass,
        other_plate_mass_center,
    );
    mov.collide(&this_mass, &mut other_plate, 456.2);

    // Expected values for float-only math (no double intermediate precision).
    expect_float_eq!(-6.2893458e-05f32, other_plate.dec_impulse_delta().x());
    expect_float_eq!(-0.00064989907f32, other_plate.dec_impulse_delta().y());
}
