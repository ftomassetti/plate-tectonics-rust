//! Port of `src/simplerandom.hpp` / `src/simplerandom.cpp`.
//!
//! The Cong linear congruential generator from
//! <https://github.com/cmcqueen/simplerandom>.
//!
//! This is the root of all reproducibility in the simulation: the exact draw
//! order *and* the exact points at which the generator is **copied** determine
//! the output. The C++ class has a copy constructor and is frequently passed
//! **by value** (see `Movement::Movement`, `lithosphere::createNoise`), which
//! deliberately leaves the caller's generator un-advanced. `Copy` here models
//! that faithfully — pass by value wherever the C++ does.

/// Cong LCG: `cong = 69069 * cong + 12345` (mod 2^32).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimpleRandom {
    cong: u32,
}

impl SimpleRandom {
    pub fn new(seed: u32) -> Self {
        // All state values are valid for Cong, so no sanitising is needed.
        Self { cong: seed }
    }

    pub fn next(&mut self) -> u32 {
        // Wrapping is load-bearing: the C++ relies on unsigned overflow here.
        self.cong = 69069u32.wrapping_mul(self.cong).wrapping_add(12345);
        self.cong
    }

    /// Return a random value in [0.0, 1.0].
    pub fn next_double(&mut self) -> f64 {
        let n = self.next();
        n as f64 / self.maximum() as f64
    }

    /// Return a random value in [0.0, 1.0].
    pub fn next_float(&mut self) -> f32 {
        let n = self.next();
        // `4294967295u32 as f32` rounds to 2^32, exactly as C++ `(float)maximum()` does.
        n as f32 / self.maximum() as f32
    }

    /// Return a random value in [-0.5, 0.5].
    pub fn next_float_signed(&mut self) -> f32 {
        let value = self.next_float();
        crate::platec_assert!(
            (0.0..=1.0).contains(&value),
            "Invalid float range"
        );
        value - 0.5
    }

    pub fn next_signed(&mut self) -> i32 {
        self.next() as i32
    }

    pub fn maximum(&self) -> u32 {
        4294967295
    }
}
