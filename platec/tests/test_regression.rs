//! Port of `test/test_regression.cpp`.
//!
//! Statistical comparison of heightmap data, to detect meaningful changes while
//! tolerating minor floating-point differences across platforms. The baselines
//! are inline in the C++ source and copied verbatim here; the PNG artifact
//! writing of the original is not ported.
//!
//! This runs a full 600×400 simulation to completion, so it is slow in a debug
//! build — run it with `cargo test --release`.

// The port is mechanical, and several lints fire on constructs kept verbatim
// from the C++ for numerical fidelity (literal precision, `-1.0f * x`,
// negated float comparisons, and so on).
#![allow(clippy::excessive_precision)]
#![allow(clippy::manual_abs_diff)]
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::neg_cmp_op_on_partial_ord)]
#![allow(clippy::neg_multiply)]
#![allow(clippy::should_implement_trait)]
// Only meaningful against the bit-exact C++ behaviour: the default build
// deliberately diverges from it (see the `classic_cpp` feature), so the whole
// file compiles away unless that feature is on. Run it with
// `cargo test --release --features platec/classic_cpp`.
#![cfg(feature = "classic_cpp")]

use platec::api::Simulation;

/// Statistical summary of heightmap data.
#[derive(Clone, Copy, Debug)]
struct HeightmapStats {
    min: f32,
    max: f32,
    mean: f32,
    median: f32,
    std_dev: f32,
    /// 25th percentile.
    q25: f32,
    /// 75th percentile.
    q75: f32,
}

fn compute_stats(heightmap: &[f32]) -> HeightmapStats {
    let size = heightmap.len();

    // Copy the data for sorting (to find the median and quantiles).
    let mut sorted_data = heightmap.to_vec();
    sorted_data.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let min = sorted_data[0];
    let max = sorted_data[size - 1];
    let median = sorted_data[size / 2];
    let q25 = sorted_data[size / 4];
    let q75 = sorted_data[(3 * size) / 4];

    // The C++ accumulates the mean and variance in `double`.
    let mut sum = 0.0f64;
    for &v in heightmap {
        sum += v as f64;
    }
    let mean = (sum / size as f64) as f32;

    let mut variance = 0.0f64;
    for &v in heightmap {
        let diff = v as f64 - mean as f64;
        variance += diff * diff;
    }
    let std_dev = (variance / size as f64).sqrt() as f32;

    HeightmapStats {
        min,
        max,
        mean,
        median,
        std_dev,
        q25,
        q75,
    }
}

/// Relative tolerance, with an absolute floor for values near zero.
fn close_enough(a: f32, b: f32, rel_tol: f32) -> bool {
    let abs_tolerance = 0.05f32.max(b.abs() * rel_tol);
    (a - b).abs() <= abs_tolerance
}

fn stats_match(
    actual: &HeightmapStats,
    expected: &HeightmapStats,
    central_tolerance: f32,
    extrema_tolerance: f32,
) -> bool {
    // Min/max are single extreme values, more sensitive to platform
    // differences, so they get a more generous tolerance.
    let extrema_ok = close_enough(actual.min, expected.min, extrema_tolerance)
        && close_enough(actual.max, expected.max, extrema_tolerance);

    // Central tendency metrics are more stable: stricter tolerance.
    let central_ok = close_enough(actual.mean, expected.mean, central_tolerance)
        && close_enough(actual.median, expected.median, central_tolerance)
        && close_enough(actual.std_dev, expected.std_dev, central_tolerance)
        && close_enough(actual.q25, expected.q25, central_tolerance)
        && close_enough(actual.q75, expected.q75, central_tolerance);

    extrema_ok && central_ok
}

#[test]
fn regression_simulation_seed12345_output_consistency() {
    let seed = 12345u32;
    let width = 600u32;
    let height = 400u32;

    // Create the simulation with the same parameters as the baseline.
    let mut sim = Simulation::create(seed, width, height, 0.65, 60, 0.02, 1_000_000, 0.33, 2, 10)
        .expect("Failed to create simulation");

    let initial_stats = compute_stats(sim.heightmap());

    // Run the simulation to completion.
    while !sim.is_finished() {
        sim.step();
    }

    let final_stats = compute_stats(sim.heightmap());

    // Adaptive tolerances: strict for central tendency, generous for extrema.
    let central_tolerance = 0.01f32; // 1% for mean, median, std_dev, quantiles
    let extrema_tolerance = 0.15f32; // 15% for min/max

    // Expected statistical properties from baseline runs with seed 12345. The
    // initial state is identical across platforms (generated before the
    // simulation runs).
    let expected_initial = HeightmapStats {
        min: 0.1,
        max: 2.0,
        mean: 0.689232,
        median: 0.1,
        std_dev: 0.779593,
        q25: 0.1,
        q75: 1.63843,
    };

    // The final state differs across architectures due to floating-point
    // accumulation. Baseline: macOS ARM64 (Apple Clang, NEON).
    let expected_final_arm64 = HeightmapStats {
        min: 0.0390768,
        max: 12.8598,
        mean: 0.624101,
        median: 0.114091,
        std_dev: 0.930413,
        q25: 0.0982555,
        q75: 0.958133,
    };

    // Baseline: Windows/Ubuntu x86-64 (MSVC/GCC, AVX2/SSE).
    let expected_final_x86 = HeightmapStats {
        min: 0.0423916,
        max: 17.8405,
        mean: 0.62406,
        median: 0.114578,
        std_dev: 0.945673,
        q25: 0.0983445,
        q75: 0.924061,
    };

    let initial_matches = stats_match(
        &initial_stats,
        &expected_initial,
        central_tolerance,
        extrema_tolerance,
    );
    let final_matches_arm64 = stats_match(
        &final_stats,
        &expected_final_arm64,
        central_tolerance,
        extrema_tolerance,
    );
    let final_matches_x86 = stats_match(
        &final_stats,
        &expected_final_x86,
        central_tolerance,
        extrema_tolerance,
    );
    let final_matches = final_matches_arm64 || final_matches_x86;

    println!("=== Initial heightmap statistics ===\n{initial_stats:#?}");
    println!("=== Final heightmap statistics ===\n{final_stats:#?}");
    println!(
        "matched baseline: {}",
        if final_matches_arm64 {
            "ARM64"
        } else if final_matches_x86 {
            "x86-64"
        } else {
            "none"
        }
    );

    assert!(
        initial_matches,
        "Initial heightmap statistics differ significantly from baseline.\n\
         actual: {initial_stats:#?}\nexpected: {expected_initial:#?}"
    );
    assert!(
        final_matches,
        "Final heightmap statistics differ significantly from both baselines.\n\
         actual: {final_stats:#?}\nARM64: {expected_final_arm64:#?}\nx86: {expected_final_x86:#?}"
    );
}
