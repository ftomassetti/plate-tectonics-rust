//! Tests for the 2D map buffer.
//!
//! The `IndexedAccessOperatorFromWorldPoint` case is
//! not ported (it was never enabled).

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

use platec::heightmap::HeightMap;

fn filled() -> HeightMap {
    let mut hm = HeightMap::new(50, 20);
    hm.set(0, 0, 0.2);
    hm.set(20, 18, 0.7);
    hm.set(40, 18, 0.5);
    hm.set(49, 19, 0.9);
    hm
}

#[test]
fn heightmap_constructor_width_height() {
    let hm = HeightMap::new(50, 20);
    assert_eq!(50, hm.width());
    assert_eq!(20, hm.height());
}

#[test]
fn heightmap_area() {
    let hm = HeightMap::new(50, 20);
    assert_eq!(1000, hm.area());
}

#[test]
fn heightmap_set_and_get() {
    let hm = filled();
    assert_eq!(0.2f32, *hm.get(0, 0));
    assert_eq!(0.7f32, *hm.get(20, 18));
    assert_eq!(0.5f32, *hm.get(40, 18));
    assert_eq!(0.9f32, *hm.get(49, 19));
}

#[test]
fn heightmap_copy_constructor() {
    let hm = filled();
    let hm2 = hm.clone();
    assert_eq!(0.2f32, *hm2.get(0, 0));
    assert_eq!(0.7f32, *hm2.get(20, 18));
    assert_eq!(0.5f32, *hm2.get(40, 18));
    assert_eq!(0.9f32, *hm2.get(49, 19));
}

#[test]
fn heightmap_assignment_operator() {
    let hm = filled();
    // Assignment reallocates when the areas differ, adopting
    // the source's dimensions.
    let mut hm2 = HeightMap::new(10, 10);
    hm2.copy_from(&hm);
    assert_eq!(0.2f32, *hm2.get(0, 0));
    assert_eq!(0.7f32, *hm2.get(20, 18));
    assert_eq!(0.5f32, *hm2.get(40, 18));
    assert_eq!(0.9f32, *hm2.get(49, 19));
}

#[test]
fn heightmap_set_all() {
    let mut hm = HeightMap::new(50, 20);
    hm.set_all(1.789);
    assert_eq!(1.789f32, *hm.get(0, 0));
    assert_eq!(1.789f32, *hm.get(20, 18));
    assert_eq!(1.789f32, *hm.get(40, 18));
    assert_eq!(1.789f32, *hm.get(49, 19));
}

#[test]
fn heightmap_indexed_access_operator_from_index() {
    let mut hm = filled();

    assert_eq!(0.2f32, hm[0usize]);
    assert_eq!(0.7f32, hm[920usize]);
    assert_eq!(0.5f32, hm[940usize]);
    assert_eq!(0.9f32, hm[999usize]);

    hm[0usize] += 0.1;
    hm[920usize] += 0.1;
    hm[940usize] -= 0.1;
    hm[999usize] -= 0.1;

    expect_float_eq!(0.3f32, hm[0usize]);
    expect_float_eq!(0.8f32, hm[920usize]);
    expect_float_eq!(0.4f32, hm[940usize]);
    expect_float_eq!(0.8f32, hm[999usize]);
}
