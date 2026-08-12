//! Test helpers shared by the test suites.
#![allow(dead_code)]
#![allow(unused_macros)]

/// gtest's `SignAndMagnitudeToBiased` for `f32`.
fn biased_f32(value: f32) -> u32 {
    const SIGN_BIT: u32 = 1 << 31;
    let bits = value.to_bits();
    if bits & SIGN_BIT != 0 {
        (!bits).wrapping_add(1)
    } else {
        SIGN_BIT | bits
    }
}

fn biased_f64(value: f64) -> u64 {
    const SIGN_BIT: u64 = 1 << 63;
    let bits = value.to_bits();
    if bits & SIGN_BIT != 0 {
        (!bits).wrapping_add(1)
    } else {
        SIGN_BIT | bits
    }
}

/// gtest's `AlmostEquals`: within 4 ULPs, NaN never equal.
pub fn almost_equals_f32(a: f32, b: f32) -> bool {
    if a.is_nan() || b.is_nan() {
        return false;
    }
    let (x, y) = (biased_f32(a), biased_f32(b));
    let distance = if x >= y { x - y } else { y - x };
    distance <= 4
}

pub fn almost_equals_f64(a: f64, b: f64) -> bool {
    if a.is_nan() || b.is_nan() {
        return false;
    }
    let (x, y) = (biased_f64(a), biased_f64(b));
    let distance = if x >= y { x - y } else { y - x };
    distance <= 4
}

/// Equivalent of gtest's `EXPECT_FLOAT_EQ`.
macro_rules! expect_float_eq {
    ($expected:expr, $actual:expr) => {{
        let expected: f32 = $expected;
        let actual: f32 = $actual;
        assert!(
            crate::common::almost_equals_f32(expected, actual),
            "expected {:?} ({:#x}) to be within 4 ULPs of {:?} ({:#x})",
            expected,
            expected.to_bits(),
            actual,
            actual.to_bits()
        );
    }};
}

/// Equivalent of gtest's `EXPECT_DOUBLE_EQ`.
macro_rules! expect_double_eq {
    ($expected:expr, $actual:expr) => {{
        let expected: f64 = $expected;
        let actual: f64 = $actual;
        assert!(
            crate::common::almost_equals_f64(expected, actual),
            "expected {:?} to be within 4 ULPs of {:?}",
            expected,
            actual
        );
    }};
}
