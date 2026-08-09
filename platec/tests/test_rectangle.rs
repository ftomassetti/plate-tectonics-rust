//! Port of `test/test_rectangle.cpp`.

// The port is mechanical, and several lints fire on constructs kept verbatim
// from the C++ for numerical fidelity (literal precision, `-1.0f * x`,
// negated float comparisons, and so on).
#![allow(clippy::excessive_precision)]
#![allow(clippy::manual_abs_diff)]
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::neg_cmp_op_on_partial_ord)]
#![allow(clippy::neg_multiply)]
#![allow(clippy::should_implement_trait)]

use platec::geometry::WorldDimension;
use platec::rectangle::{Rectangle, BAD_INDEX};

#[test]
fn rectangle_map_index_inside_rect_not_wrapping() {
    let r = Rectangle::new(WorldDimension::new(50, 30), 42, 48, 8, 15);

    let (mut px, mut py) = (42u32, 8u32);
    let res = r.get_map_index(&mut px, &mut py);
    assert_eq!(px, 0);
    assert_eq!(py, 0);
    assert_eq!(res, 0);

    let (mut px, mut py) = (43u32, 8u32);
    let res = r.get_map_index(&mut px, &mut py);
    assert_eq!(px, 1);
    assert_eq!(py, 0);
    assert_eq!(res, 1);

    let (mut px, mut py) = (42u32, 9u32);
    let res = r.get_map_index(&mut px, &mut py);
    assert_eq!(px, 0);
    assert_eq!(py, 1);
    assert_eq!(res, 6);
}

#[test]
fn rectangle_map_index_outside_rect() {
    let r = Rectangle::new(WorldDimension::new(50, 30), 42, 48, 8, 15);

    // On failure the coordinates are left untouched.
    let (mut px, mut py) = (49u32, 8u32);
    let res = r.get_map_index(&mut px, &mut py);
    assert_eq!(px, 49);
    assert_eq!(py, 8);
    assert_eq!(res, BAD_INDEX);

    let (mut px, mut py) = (48u32, 15u32);
    let res = r.get_map_index(&mut px, &mut py);
    assert_eq!(px, 48);
    assert_eq!(py, 15);
    assert_eq!(res, BAD_INDEX);

    let (mut px, mut py) = (2u32, 2u32);
    let res = r.get_map_index(&mut px, &mut py);
    assert_eq!(px, 2);
    assert_eq!(py, 2);
    assert_eq!(res, BAD_INDEX);
}

#[test]
fn rectangle_map_index_inside_rect_wrapping_on_x() {
    let r = Rectangle::new(WorldDimension::new(50, 30), 42, 6, 8, 12);

    let (mut px, mut py) = (42u32, 8u32);
    let res = r.get_map_index(&mut px, &mut py);
    assert_eq!(px, 0);
    assert_eq!(py, 0);
    assert_eq!(res, 0);

    let (mut px, mut py) = (0u32, 8u32);
    let res = r.get_map_index(&mut px, &mut py);
    assert_eq!(px, 8);
    assert_eq!(py, 0);
    assert_eq!(res, 8);

    let (mut px, mut py) = (2u32, 9u32);
    let res = r.get_map_index(&mut px, &mut py);
    assert_eq!(px, 10);
    assert_eq!(py, 1);
    assert_eq!(res, 24);
}

#[test]
fn rectangle_map_index_inside_rect_wrapping_on_y() {
    let r = Rectangle::new(WorldDimension::new(50, 30), 42, 48, 25, 5);

    let (mut px, mut py) = (42u32, 25u32);
    let res = r.get_map_index(&mut px, &mut py);
    assert_eq!(px, 0);
    assert_eq!(py, 0);
    assert_eq!(res, 0);

    let (mut px, mut py) = (44u32, 29u32);
    let res = r.get_map_index(&mut px, &mut py);
    assert_eq!(px, 2);
    assert_eq!(py, 4);
    assert_eq!(res, 26);

    let (mut px, mut py) = (44u32, 2u32);
    let res = r.get_map_index(&mut px, &mut py);
    assert_eq!(px, 2);
    assert_eq!(py, 7);
    assert_eq!(res, 44);
}

#[test]
fn rectangle_map_index_inside_rect_large_as_world() {
    let r = Rectangle::new(WorldDimension::new(50, 30), 0, 50, 0, 30);

    let (mut px, mut py) = (0u32, 0u32);
    let res = r.get_map_index(&mut px, &mut py);
    assert_eq!(px, 0);
    assert_eq!(py, 0);
    assert_eq!(res, 0);

    let (mut px, mut py) = (12u32, 8u32);
    let res = r.get_map_index(&mut px, &mut py);
    assert_eq!(px, 12);
    assert_eq!(py, 8);
    assert_eq!(res, 412);

    let (mut px, mut py) = (49u32, 29u32);
    let res = r.get_map_index(&mut px, &mut py);
    assert_eq!(px, 49);
    assert_eq!(py, 29);
    assert_eq!(res, 1499);
}
