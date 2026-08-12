//! Acceptance tests.
//!
//! These acceptance tests were originally derived by running platec (the
//! original library); the goal was to refactor the code while obtaining the
//! same results. **Both cases are disabled** because
//! they are platform dependent — they only ever held on Linux/Travis. They are
//! ported here as `#[ignore]`d tests so the expected values stay recorded and
//! can be checked deliberately with `cargo test --release -- --ignored`.

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

use platec::api::Simulation;

fn sim() -> Simulation {
    Simulation::create(3, 512, 512, 0.65, 60, 0.02, 1_000_000, 0.33, 2, 10).unwrap()
}

#[test]
#[ignore = "platform dependent"]
fn platec_create_same_result_as_platec() {
    let p = sim();
    let heightmap = p.heightmap();

    expect_float_eq!(0.1f32, heightmap[0]);
    expect_float_eq!(1.483476f32, heightmap[100]);
    expect_float_eq!(0.1f32, heightmap[200]);
    expect_float_eq!(1.5217931f32, heightmap[1000]);
    expect_float_eq!(0.1f32, heightmap[5000]);
    expect_float_eq!(1.4538962f32, heightmap[50000]);
    expect_float_eq!(1.5340517f32, heightmap[100000]);
    expect_float_eq!(0.1f32, heightmap[150000]);
    expect_float_eq!(1.492384f32, heightmap[200000]);
    expect_float_eq!(0.1f32, heightmap[250000]);
    expect_float_eq!(0.1f32, heightmap[262143]);
}

#[test]
#[ignore = "platform dependent"]
fn platec_global_generation_same_result_as_platec() {
    let mut p = sim();
    while !p.is_finished() {
        p.step();
    }
    let heightmap = p.heightmap();

    expect_float_eq!(2.0832214f32, heightmap[0]);
    expect_float_eq!(0.089928061f32, heightmap[100]);
    expect_float_eq!(0.092105672f32, heightmap[200]);
    expect_float_eq!(0.10110986f32, heightmap[1000]);
    expect_float_eq!(0.10199541f32, heightmap[5000]);
    expect_float_eq!(0.11336569f32, heightmap[50000]);
    expect_float_eq!(1.0184617f32, heightmap[100000]);
    expect_float_eq!(0.84557754f32, heightmap[150000]);
    expect_float_eq!(0.12073194f32, heightmap[200000]);
    expect_float_eq!(0.12088145f32, heightmap[250000]);
    expect_float_eq!(2.1296265f32, heightmap[262143]);
}
