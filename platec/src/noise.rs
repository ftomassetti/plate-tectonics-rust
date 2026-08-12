//! Port of `src/noise.hpp` / `src/noise.cpp`.
//!
//! Both entry points take the generator **by value**, exactly as the C++ does:
//! the caller's generator is deliberately left un-advanced, and `lithosphere`
//! depends on that.

use crate::geometry::WorldDimension;
use crate::simplerandom::SimpleRandom;
use crate::simplexnoise::{scaled_octave_noise_4d, simplexnoise};
use crate::sqrdmd::sqrdmd;
use crate::utils::PI;

const SQRDMD_ROUGHNESS: f32 = 0.35;

fn nearest_pow(num: u32) -> u32 {
    let mut n = 1u32;
    while n < num {
        n <<= 1;
    }
    n
}

pub fn create_slow_noise(map: &mut [f32], tmp_dim: &WorldDimension, mut randsource: SimpleRandom) {
    // `next()` is a u32, so the seed is always non-negative here.
    let seed: i64 = randsource.next() as i64;
    let width = tmp_dim.get_width();
    let height = tmp_dim.get_height();
    let persistence = 0.25f32;
    let noise_scale = 0.593f32;
    let ka = (256 / seed) as f32;
    let kb = (seed * 567 % 256) as f32;
    // `seed * seed` overflows int64_t for seeds above ~3.03e9 — undefined
    // behaviour in C++ that wraps in practice, so wrap explicitly.
    let kc = (seed.wrapping_mul(seed) % 256) as f32;
    let kd = ((567 - seed) % 256) as f32;

    for y in 0..height {
        for x in 0..width {
            // The offsets define a circle each; a full circle is two pi radians,
            // and one full circle across the axis is what makes the result tile.
            //
            // The C++ sweeps y over *four* pi, i.e. two full circles, so the
            // noise it produces has period height/2 and the bottom half of every
            // world is a copy of the top half. `classic_cpp` keeps that.
            let f_nx = x as f32 / width as f32;
            let f_ny = y as f32 / height as f32;
            let f_rdx = f_nx * 2.0 * PI;
            let f_rdy = if cfg!(feature = "classic_cpp") {
                f_ny * 4.0 * PI
            } else {
                f_ny * 2.0 * PI
            };
            let f_rds_sin = 1.0f32;
            let a = f_rds_sin * f_rdx.sin();
            let b = f_rds_sin * f_rdx.cos();
            let c = f_rds_sin * f_rdy.sin();
            let d = f_rds_sin * f_rdy.cos();
            let v = scaled_octave_noise_4d(
                4.0,
                persistence,
                0.25,
                0.0,
                1.0,
                ka + a * noise_scale,
                kb + b * noise_scale,
                kc + c * noise_scale,
                kd + d * noise_scale,
            );
            map[(y * width + x) as usize] = v;
        }
    }
}

pub fn create_noise(
    tmp: &mut [f32],
    tmp_dim: &WorldDimension,
    mut randsource: SimpleRandom,
    use_simplex: bool,
) {
    let width = tmp_dim.get_width() as usize;
    let height = tmp_dim.get_height() as usize;

    if use_simplex {
        simplexnoise(
            randsource.next() as i32,
            tmp,
            tmp_dim.get_width() as i32,
            tmp_dim.get_height() as i32,
            SQRDMD_ROUGHNESS,
        );
        return;
    }

    let side = (nearest_pow(tmp_dim.get_max()) + 1) as usize;
    let mut square_tmp = vec![0.0f32; side * side];
    for y in 0..height {
        square_tmp[y * side..y * side + width].copy_from_slice(&tmp[y * width..y * width + width]);
    }

    // To make it tileable we need to insert proper values in the padding area.
    // 1) On the right of the valid area: a mix between the east and west
    //    borders (they should be fairly similar because the world is toroidal).
    for y in 0..height {
        for x in width..side {
            square_tmp[y * side + x] = (square_tmp[y * side] + square_tmp[y * side + (width - 1)]) / 2.0;
        }
    }
    // 2) Below the valid area: a mix between the north and south borders.
    for y in height..side {
        for x in 0..side {
            square_tmp[y * side + x] = (square_tmp[x] + square_tmp[(height - 1) * side + x]) / 2.0;
        }
    }

    sqrdmd(
        randsource.next(),
        &mut square_tmp,
        side as i32,
        SQRDMD_ROUGHNESS,
    );

    // Calculate deltas (noise introduced).
    let mut deltas = vec![0.0f32; width * height];
    for y in 0..height {
        for x in 0..width {
            deltas[y * width + x] = square_tmp[y * side + x] - tmp[y * width + x];
        }
    }

    // Make it tileable.
    for y in 0..height {
        for x in 0..width {
            let specular_x = width - 1 - x;
            let specular_y = height - 1 - y;
            let my_delta = deltas[y * width + x];
            let specular_width_delta = deltas[y * width + specular_x];
            let specular_height_delta = deltas[specular_y * width + x];
            let opposite_delta = deltas[specular_y * width + specular_x];
            tmp[y * width + x] +=
                (my_delta + specular_width_delta + specular_height_delta + opposite_delta) / 4.0;
        }
    }
}
