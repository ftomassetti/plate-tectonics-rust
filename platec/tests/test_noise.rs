//! The noise-related cases from `test/test_plate.cpp` (`SimpleRandom.NextRepeatability`,
//! `Noise.SimplexRawNoiseRepeatability`, `Noise.SimplexNoiseRepeatability`).
//!
//! They live in their own file here so that the noise stack can be validated
//! before `plate` exists.

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

use platec::geometry::WorldDimension;
use platec::noise::create_noise;
use platec::simplerandom::SimpleRandom;
use platec::simplexnoise::raw_noise_4d;

/// Port of the `initializeHeightmapWithNoise` helper at the top of test_plate.cpp.
pub fn initialize_heightmap_with_noise(seed: u32, heightmap: &mut [f32], wd: &WorldDimension) {
    create_noise(heightmap, wd, SimpleRandom::new(seed), true);
    for v in heightmap.iter_mut().take(wd.get_area() as usize) {
        if *v < 0.0 {
            *v *= -1.0;
        }
    }
}

#[test]
fn simple_random_next_repeatability() {
    let mut sr1 = SimpleRandom::new(1);
    assert_eq!(81414, sr1.next());
    assert_eq!(1328228615, sr1.next());
    assert_eq!(3215746516, sr1.next());

    let mut sr999 = SimpleRandom::new(999);
    assert_eq!(69012276, sr999.next());
    assert_eq!(3490172125, sr999.next());
    assert_eq!(3364058674, sr999.next());
}

#[test]
fn noise_simplex_raw_noise_repeatability() {
    expect_float_eq!(-0.12851511f32, raw_noise_4d(0.3, 0.78, 1.677, 0.99));
    expect_float_eq!(-0.83697641f32, raw_noise_4d(-0.3, 0.78, 1.677, 0.99));
    expect_float_eq!(-0.5346418f32, raw_noise_4d(7339.3, 0.78, 1.677, 0.99));
    expect_float_eq!(0.089452535f32, raw_noise_4d(0.3, 70.78, 1.677, 0.0009));
    expect_float_eq!(-0.063593753f32, raw_noise_4d(0.3, 500.78, 1.677, 500.99));
}

#[test]
fn noise_simplex_noise_repeatability() {
    let wd = WorldDimension::new(233, 111);
    let mut heightmap = vec![0.0f32; wd.get_area() as usize];
    initialize_heightmap_with_noise(123, &mut heightmap, &wd);

    expect_float_eq!(0.50098729f32, heightmap[0]);
    expect_float_eq!(0.39222634f32, heightmap[1000]);
    expect_float_eq!(0.51659518f32, heightmap[2000]);
    expect_float_eq!(0.5479334f32, heightmap[5000]);
    expect_float_eq!(0.59222502f32, heightmap[8000]);
    expect_float_eq!(0.36362505f32, heightmap[11000]);
    expect_float_eq!(0.57599854f32, heightmap[13000]);
}
