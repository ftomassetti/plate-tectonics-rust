//! Port of `test/test_mass.cpp`.

// The port is mechanical, and several lints fire on constructs kept verbatim
// from the C++ for numerical fidelity (literal precision, `-1.0f * x`,
// negated float comparisons, and so on).
#![allow(clippy::excessive_precision)]
#![allow(clippy::manual_abs_diff)]
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::neg_cmp_op_on_partial_ord)]
#![allow(clippy::neg_multiply)]
#![allow(clippy::should_implement_trait)]

#[macro_use]
mod common;

use platec::geometry::Dimension;
use platec::mass::{Mass, MassBuilder, MassLike};

#[test]
fn mass_builder_constructor_from_heightmap() {
    let heightmap: [f32; 20] = [
        0.0, 0.0, 0.0, 0.0, 10.3, //
        5.0, 0.0, 0.0, 0.0, 0.0, //
        0.2, 0.0, 0.0, 0.0, 0.0, //
        1.0, 1.0, 0.0, 0.0, 0.0,
    ];
    let dim = Dimension::new(5, 4);
    let mb = MassBuilder::from_slice(&heightmap, &dim);
    expect_float_eq!(17.5f32, mb.build().get_mass());
    expect_float_eq!(2.4114285714285715f32, mb.build().get_cx());
    expect_float_eq!(0.6514285714285715f32, mb.build().get_cy());
}

#[test]
fn mass_builder_add_point() {
    let mut mb = MassBuilder::new();
    expect_float_eq!(0.0f32, mb.build().get_mass());

    mb.add_point(10, 10, 123.0);
    expect_float_eq!(123.0f32, mb.build().get_mass());
    expect_float_eq!(10.0f32, mb.build().get_cx());
    expect_float_eq!(10.0f32, mb.build().get_cy());

    mb.add_point(0, 5, 123.0);
    expect_float_eq!(246.0f32, mb.build().get_mass());
    expect_float_eq!(5.0f32, mb.build().get_cx());
    expect_float_eq!(7.5f32, mb.build().get_cy());
}

#[test]
fn mass_constructor() {
    let mass1 = Mass::new(0.0, 7.5, 8.5);
    expect_float_eq!(0.0f32, mass1.get_mass());

    let mass2 = Mass::new(8.5, 7.6, 27.5);
    expect_float_eq!(8.5f32, mass2.get_mass());
    expect_float_eq!(7.6f32, mass2.get_cx());
    expect_float_eq!(27.5f32, mass2.get_cy());
}

#[test]
fn mass_null() {
    let mass1 = Mass::new(0.0, 7.5, 8.5);
    assert!(mass1.null());

    let mass2 = Mass::new(8.5, 7.6, 27.5);
    assert!(!mass2.null());
}

#[test]
fn mass_inc_mass() {
    let mut mass = Mass::new(8.5, 7.6, 27.5);
    expect_float_eq!(8.5f32, mass.get_mass());
    expect_float_eq!(7.6f32, mass.get_cx());
    expect_float_eq!(27.5f32, mass.get_cy());

    mass.inc_mass(10.0);
    expect_float_eq!(18.5f32, mass.get_mass());
    expect_float_eq!(7.6f32, mass.get_cx());
    expect_float_eq!(27.5f32, mass.get_cy());

    mass.inc_mass(-18.0);
    expect_float_eq!(0.5f32, mass.get_mass());
    expect_float_eq!(7.6f32, mass.get_cx());
    expect_float_eq!(27.5f32, mass.get_cy());
}
