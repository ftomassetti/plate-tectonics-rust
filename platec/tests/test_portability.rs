//! Tests that arithmetic behaves the same across platforms.
//!
//! We want floats to behave consistently across platforms.

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

use std::hint::black_box;

#[test]
fn portability_float_ops() {
    let mut v: f32 = 123456.789012;
    // `black_box` stops the compiler folding this; it is the
    // Rust equivalent of "do not constant-fold this".
    let constant: f32 = black_box(812345.0123);

    for _ in 0..2 {
        v *= v + constant;
    }
    expect_float_eq!(1.3347527e+22f32, v);

    for _ in 0..3 {
        v *= v + constant;
    }
    expect_float_eq!(f32::INFINITY, v);

    for _ in 0..95 {
        v *= v + constant;
    }
    expect_float_eq!(f32::INFINITY, v);
}

#[test]
fn randomness_double_ops() {
    // Note this initialises an `f64` from an `f32` literal (`123456.789012`),
    // and likewise adds a float literal each iteration. Both are reproduced here.
    let mut v: f64 = 123456.789012f32 as f64;
    let constant: f64 = 812345.0123f32 as f64;

    for _ in 0..2 {
        v *= v + constant;
    }
    expect_double_eq!(1.3347525239012724e+22f64, v);

    for _ in 0..3 {
        v *= v + constant;
    }
    expect_double_eq!(1.0074094163955063e+177f64, v);

    for _ in 0..95 {
        v *= v + constant;
    }
    expect_double_eq!(f64::INFINITY, v);
}
