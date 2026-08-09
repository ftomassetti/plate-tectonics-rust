//! Port of `test/test_bounds.cpp`.

// The port is mechanical, and several lints fire on constructs kept verbatim
// from the C++ for numerical fidelity (literal precision, `-1.0f * x`,
// negated float comparisons, and so on).
#![allow(clippy::excessive_precision)]
#![allow(clippy::manual_abs_diff)]
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::neg_cmp_op_on_partial_ord)]
#![allow(clippy::neg_multiply)]
#![allow(clippy::should_implement_trait)]

use platec::bounds::Bounds;
use platec::geometry::{Dimension, FloatPoint, WorldDimension};
use platec::rectangle::BAD_INDEX;

fn wd() -> WorldDimension {
    WorldDimension::new(800, 600)
}
fn top_left() -> FloatPoint {
    FloatPoint::new(10.2, 48.9)
}
fn plate_dim() -> Dimension {
    Dimension::new(500, 400)
}
fn b() -> Bounds {
    Bounds::new(wd(), top_left(), plate_dim())
}

#[test]
fn bounds_index() {
    let b = b();
    assert_eq!(0, b.index(0, 0));
    assert_eq!(100100, b.index(100, 200));
    assert_eq!(199999, b.index(499, 399));
}

#[test]
fn bounds_area() {
    assert_eq!(200000, b().area());
}

#[test]
fn bounds_width() {
    assert_eq!(500, b().width());
}

#[test]
fn bounds_height() {
    assert_eq!(400, b().height());
}

#[test]
fn bounds_left_as_uint() {
    assert_eq!(10, b().left_as_uint());
}

#[test]
fn bounds_top_as_uint() {
    assert_eq!(48, b().top_as_uint());
}

#[test]
fn bounds_right_as_uint_non_inclusive() {
    assert_eq!(509, b().right_as_uint_non_inclusive());
}

#[test]
fn bounds_bottom_as_uint_non_inclusive() {
    assert_eq!(447, b().bottom_as_uint_non_inclusive());
}

#[test]
fn bounds_contains_world_point() {
    let b = b();

    // world corners
    assert!(!b.contains_world_point(0, 0));
    assert!(!b.contains_world_point(799, 0));
    assert!(!b.contains_world_point(0, 599));
    assert!(!b.contains_world_point(799, 599));

    // plate corners
    assert!(b.contains_world_point(10, 48));
    assert!(b.contains_world_point(509, 48));
    assert!(b.contains_world_point(10, 447));
    assert!(b.contains_world_point(509, 447));

    // inside plate
    assert!(b.contains_world_point(10, 48));
    assert!(b.contains_world_point(120, 100));
    assert!(b.contains_world_point(400, 400));
    assert!(b.contains_world_point(509, 447));

    // outside plate
    assert!(!b.contains_world_point(10, 0));
    assert!(!b.contains_world_point(10, 47));
    assert!(!b.contains_world_point(10, 448));
    assert!(!b.contains_world_point(10, 490));
    assert!(!b.contains_world_point(100, 0));
    assert!(!b.contains_world_point(100, 47));
    assert!(!b.contains_world_point(100, 448));
    assert!(!b.contains_world_point(100, 490));
    assert!(!b.contains_world_point(509, 0));
    assert!(!b.contains_world_point(509, 47));
    assert!(!b.contains_world_point(509, 448));
    assert!(!b.contains_world_point(509, 490));
}

#[test]
fn bounds_is_in_limits() {
    let b = b();

    // negative coordinates
    assert!(!b.is_in_limits(-1.0, 10.0));
    assert!(!b.is_in_limits(10.0, -1.0));
    assert!(!b.is_in_limits(-1.0, -1.0));

    assert!(b.is_in_limits(0.0, 0.0));
    assert!(b.is_in_limits(124.3, 245.56));
    assert!(b.is_in_limits(499.0, 399.0));
    assert!(b.is_in_limits(499.1, 399.1));
    assert!(b.is_in_limits(499.999, 399.999));

    assert!(!b.is_in_limits(500.0, 399.0));
    assert!(!b.is_in_limits(499.0, 400.0));
    assert!(!b.is_in_limits(500.0, 400.0));
}

#[test]
fn bounds_shift() {
    let mut bounds = b();
    // topLeft = 10.2, 48.9
    bounds.shift(10.7, 100.1);
    // now topLeft should be = 20.9, 149.0
    assert_eq!(20, bounds.left_as_uint());
    assert_eq!(149, bounds.top_as_uint());
    // width and height should not be affected
    assert_eq!(500, bounds.width());
    assert_eq!(400, bounds.height());
}

#[test]
fn bounds_grow() {
    let mut bounds = b();
    bounds.grow(123, 0);

    assert_eq!(623, bounds.width());
    // height should not be affected
    assert_eq!(400, bounds.height());
    // topLeft not be affected
    assert_eq!(10, bounds.left_as_uint());
    assert_eq!(48, bounds.top_as_uint());

    let mut bounds2 = b();
    bounds2.grow(0, 123);

    assert_eq!(523, bounds2.height());
    // width should not be affected
    assert_eq!(500, bounds2.width());
    // topLeft not be affected
    assert_eq!(10, bounds2.left_as_uint());
    assert_eq!(48, bounds2.top_as_uint());
}

#[test]
fn bounds_get_map_index() {
    let b = b();

    let (mut px, mut py) = (10u32, 48u32);
    let res = b.get_map_index(&mut px, &mut py);
    assert_eq!(px, 0);
    assert_eq!(py, 0);
    assert_eq!(res, 0);

    let (mut px, mut py) = (510u32, 48u32);
    let res = b.get_map_index(&mut px, &mut py);
    assert_eq!(px, 510);
    assert_eq!(py, 48);
    assert_eq!(res, BAD_INDEX);

    let (mut px, mut py) = (10u32, 448u32);
    let res = b.get_map_index(&mut px, &mut py);
    assert_eq!(px, 10);
    assert_eq!(py, 448);
    assert_eq!(res, BAD_INDEX);

    let (mut px, mut py) = (110u32, 98u32);
    let res = b.get_map_index(&mut px, &mut py);
    assert_eq!(px, 100);
    assert_eq!(py, 50);
    assert_eq!(res, 25100);
}

#[test]
fn bounds_get_valid_map_index() {
    let b = b();

    let (mut px, mut py) = (10u32, 48u32);
    let res = b.get_valid_map_index(&mut px, &mut py);
    assert_eq!(px, 0);
    assert_eq!(py, 0);
    assert_eq!(res, 0);

    let (mut px, mut py) = (110u32, 98u32);
    let res = b.get_valid_map_index(&mut px, &mut py);
    assert_eq!(px, 100);
    assert_eq!(py, 50);
    assert_eq!(res, 25100);
}
