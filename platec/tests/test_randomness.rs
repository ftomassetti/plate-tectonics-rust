//! Tests for the random number generator.

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

use platec::simplerandom::SimpleRandom;

#[test]
fn randomness_sequence_doubles() {
    let seed = 3;
    let mut randsource = SimpleRandom::new(seed);

    expect_float_eq!(5.1118433e-05f32, randsource.next_float());
    expect_float_eq!(0.53070194f32, randsource.next_float());
    expect_float_eq!(0.053402752f32, randsource.next_float());
}

#[test]
fn randomness_maximum() {
    let seed = 3;
    let randsource = SimpleRandom::new(seed);

    expect_float_eq!(4.2949673e+09f32, randsource.maximum() as f32);
}
