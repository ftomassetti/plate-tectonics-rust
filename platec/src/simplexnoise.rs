//! Port of `src/simplexnoise.hpp` / `src/simplexnoise.cpp`.
//!
//! 2D, 3D and 4D Simplex Noise functions returning 'random' values in (-1, 1).
//! The algorithm was originally designed by Ken Perlin; this code is adapted
//! from Stefan Gustavson's implementation.
//!
//! Everything here is `f32`, matching the C++ exactly — widening any of it to
//! `f64` would change the generated maps.

use crate::simplexnoise_tables::{GRAD3, GRAD4, PERM, SIMPLEX};
use crate::utils::PI;

/// 2D multi-octave Simplex noise.
///
/// For each octave a higher frequency / lower amplitude function is added to
/// the original. The higher the persistence [0-1], the more of each succeeding
/// octave is added.
pub fn octave_noise_2d(octaves: f32, persistence: f32, scale: f32, x: f32, y: f32) -> f32 {
    let mut total = 0.0f32;
    let mut frequency = scale;
    let mut amplitude = 1.0f32;
    // Keep track of the largest possible amplitude, because each octave adds
    // more and we need a value in [-1, 1].
    let mut max_amplitude = 0.0f32;

    let mut i = 0;
    while (i as f32) < octaves {
        total += raw_noise_2d(x * frequency, y * frequency) * amplitude;
        frequency *= 2.0;
        max_amplitude += amplitude;
        amplitude *= persistence;
        i += 1;
    }

    total / max_amplitude
}

/// 3D multi-octave Simplex noise.
pub fn octave_noise_3d(
    octaves: f32,
    persistence: f32,
    scale: f32,
    x: f32,
    y: f32,
    z: f32,
) -> f32 {
    let mut total = 0.0f32;
    let mut frequency = scale;
    let mut amplitude = 1.0f32;
    let mut max_amplitude = 0.0f32;

    let mut i = 0;
    while (i as f32) < octaves {
        total += raw_noise_3d(x * frequency, y * frequency, z * frequency) * amplitude;
        frequency *= 2.0;
        max_amplitude += amplitude;
        amplitude *= persistence;
        i += 1;
    }

    total / max_amplitude
}

/// 4D multi-octave Simplex noise.
#[allow(clippy::too_many_arguments)]
pub fn octave_noise_4d(
    octaves: f32,
    persistence: f32,
    scale: f32,
    x: f32,
    y: f32,
    z: f32,
    w: f32,
) -> f32 {
    let mut total = 0.0f32;
    let mut frequency = scale;
    let mut amplitude = 1.0f32;
    let mut max_amplitude = 0.0f32;

    let mut i = 0;
    while (i as f32) < octaves {
        total += raw_noise_4d(
            x * frequency,
            y * frequency,
            z * frequency,
            w * frequency,
        ) * amplitude;
        frequency *= 2.0;
        max_amplitude += amplitude;
        amplitude *= persistence;
        i += 1;
    }

    total / max_amplitude
}

/// 2D scaled multi-octave Simplex noise: the result lies between the bounds.
pub fn scaled_octave_noise_2d(
    octaves: f32,
    persistence: f32,
    scale: f32,
    lo_bound: f32,
    hi_bound: f32,
    x: f32,
    y: f32,
) -> f32 {
    octave_noise_2d(octaves, persistence, scale, x, y) * (hi_bound - lo_bound) / 2.0
        + (hi_bound + lo_bound) / 2.0
}

/// 3D scaled multi-octave Simplex noise.
#[allow(clippy::too_many_arguments)]
pub fn scaled_octave_noise_3d(
    octaves: f32,
    persistence: f32,
    scale: f32,
    lo_bound: f32,
    hi_bound: f32,
    x: f32,
    y: f32,
    z: f32,
) -> f32 {
    octave_noise_3d(octaves, persistence, scale, x, y, z) * (hi_bound - lo_bound) / 2.0
        + (hi_bound + lo_bound) / 2.0
}

/// 4D scaled multi-octave Simplex noise.
#[allow(clippy::too_many_arguments)]
pub fn scaled_octave_noise_4d(
    octaves: f32,
    persistence: f32,
    scale: f32,
    lo_bound: f32,
    hi_bound: f32,
    x: f32,
    y: f32,
    z: f32,
    w: f32,
) -> f32 {
    octave_noise_4d(octaves, persistence, scale, x, y, z, w) * (hi_bound - lo_bound) / 2.0
        + (hi_bound + lo_bound) / 2.0
}

/// 2D scaled raw Simplex noise.
pub fn scaled_raw_noise_2d(lo_bound: f32, hi_bound: f32, x: f32, y: f32) -> f32 {
    raw_noise_2d(x, y) * (hi_bound - lo_bound) / 2.0 + (hi_bound + lo_bound) / 2.0
}

/// 3D scaled raw Simplex noise.
pub fn scaled_raw_noise_3d(lo_bound: f32, hi_bound: f32, x: f32, y: f32, z: f32) -> f32 {
    raw_noise_3d(x, y, z) * (hi_bound - lo_bound) / 2.0 + (hi_bound + lo_bound) / 2.0
}

/// 4D scaled raw Simplex noise.
pub fn scaled_raw_noise_4d(lo_bound: f32, hi_bound: f32, x: f32, y: f32, z: f32, w: f32) -> f32 {
    raw_noise_4d(x, y, z, w) * (hi_bound - lo_bound) / 2.0 + (hi_bound + lo_bound) / 2.0
}

/// 2D raw Simplex noise.
pub fn raw_noise_2d(x: f32, y: f32) -> f32 {
    // Noise contributions from the three corners.
    let (n0, n1, n2);

    // Skew the input space to determine which simplex cell we're in.
    let f2 = 0.5f32 * (3.0f32.sqrt() - 1.0);
    // Hairy factor for 2D.
    let s = (x + y) * f2;
    let i = fastfloor(x + s);
    let j = fastfloor(y + s);

    let g2 = (3.0f32 - 3.0f32.sqrt()) / 6.0;
    let t = (i + j) as f32 * g2;
    // Unskew the cell origin back to (x,y) space.
    let x0_origin = i as f32 - t;
    let y0_origin = j as f32 - t;
    // The x,y distances from the cell origin.
    let x0 = x - x0_origin;
    let y0 = y - y0_origin;

    // For the 2D case the simplex shape is an equilateral triangle. Determine
    // which simplex we are in. Offsets for the second (middle) corner in (i,j).
    let (i1, j1) = if x0 > y0 {
        (1, 0) // lower triangle, XY order: (0,0)->(1,0)->(1,1)
    } else {
        (0, 1) // upper triangle, YX order: (0,0)->(0,1)->(1,1)
    };

    // A step of (1,0) in (i,j) means a step of (1-c,-c) in (x,y), and a step of
    // (0,1) in (i,j) means a step of (-c,1-c) in (x,y), where c = (3-sqrt(3))/6.
    let x1 = x0 - i1 as f32 + g2;
    let y1 = y0 - j1 as f32 + g2;
    let x2 = x0 - 1.0 + 2.0 * g2;
    let y2 = y0 - 1.0 + 2.0 * g2;

    // Work out the hashed gradient indices of the three simplex corners.
    let ii = i & 255;
    let jj = j & 255;
    let gi0 = PERM[(ii + PERM[jj as usize]) as usize] % 12;
    let gi1 = PERM[(ii + i1 + PERM[(jj + j1) as usize]) as usize] % 12;
    let gi2 = PERM[(ii + 1 + PERM[(jj + 1) as usize]) as usize] % 12;

    // Calculate the contribution from the three corners.
    let mut t0 = 0.5f32 - x0 * x0 - y0 * y0;
    if t0 < 0.0 {
        n0 = 0.0;
    } else {
        t0 *= t0;
        // (x,y) of grad3 is used for the 2D gradient.
        n0 = t0 * t0 * dot2(&GRAD3[gi0 as usize], x0, y0);
    }

    let mut t1 = 0.5f32 - x1 * x1 - y1 * y1;
    if t1 < 0.0 {
        n1 = 0.0;
    } else {
        t1 *= t1;
        n1 = t1 * t1 * dot2(&GRAD3[gi1 as usize], x1, y1);
    }

    let mut t2 = 0.5f32 - x2 * x2 - y2 * y2;
    if t2 < 0.0 {
        n2 = 0.0;
    } else {
        t2 *= t2;
        n2 = t2 * t2 * dot2(&GRAD3[gi2 as usize], x2, y2);
    }

    // The result is scaled to return values in the interval [-1,1].
    70.0 * (n0 + n1 + n2)
}

/// 3D raw Simplex noise.
pub fn raw_noise_3d(x: f32, y: f32, z: f32) -> f32 {
    let (n0, n1, n2, n3);

    // Skew the input space to determine which simplex cell we're in.
    let f3 = 1.0f32 / 3.0;
    let s = (x + y + z) * f3;
    let i = fastfloor(x + s);
    let j = fastfloor(y + s);
    let k = fastfloor(z + s);

    let g3 = 1.0f32 / 6.0;
    let t = (i + j + k) as f32 * g3;
    let x0_origin = i as f32 - t;
    let y0_origin = j as f32 - t;
    let z0_origin = k as f32 - t;
    let x0 = x - x0_origin;
    let y0 = y - y0_origin;
    let z0 = z - z0_origin;

    // For the 3D case the simplex shape is a slightly irregular tetrahedron.
    let (i1, j1, k1, i2, j2, k2);
    if x0 >= y0 {
        if y0 >= z0 {
            // X Y Z order
            i1 = 1;
            j1 = 0;
            k1 = 0;
            i2 = 1;
            j2 = 1;
            k2 = 0;
        } else if x0 >= z0 {
            // X Z Y order
            i1 = 1;
            j1 = 0;
            k1 = 0;
            i2 = 1;
            j2 = 0;
            k2 = 1;
        } else {
            // Z X Y order
            i1 = 0;
            j1 = 0;
            k1 = 1;
            i2 = 1;
            j2 = 0;
            k2 = 1;
        }
    } else if y0 < z0 {
        // Z Y X order
        i1 = 0;
        j1 = 0;
        k1 = 1;
        i2 = 0;
        j2 = 1;
        k2 = 1;
    } else if x0 < z0 {
        // Y Z X order
        i1 = 0;
        j1 = 1;
        k1 = 0;
        i2 = 0;
        j2 = 1;
        k2 = 1;
    } else {
        // Y X Z order
        i1 = 0;
        j1 = 1;
        k1 = 0;
        i2 = 1;
        j2 = 1;
        k2 = 0;
    }

    let x1 = x0 - i1 as f32 + g3;
    let y1 = y0 - j1 as f32 + g3;
    let z1 = z0 - k1 as f32 + g3;
    let x2 = x0 - i2 as f32 + 2.0 * g3;
    let y2 = y0 - j2 as f32 + 2.0 * g3;
    let z2 = z0 - k2 as f32 + 2.0 * g3;
    let x3 = x0 - 1.0 + 3.0 * g3;
    let y3 = y0 - 1.0 + 3.0 * g3;
    let z3 = z0 - 1.0 + 3.0 * g3;

    // Work out the hashed gradient indices of the four simplex corners.
    let ii = i & 255;
    let jj = j & 255;
    let kk = k & 255;
    let gi0 = PERM[(ii + PERM[(jj + PERM[kk as usize]) as usize]) as usize] % 12;
    let gi1 =
        PERM[(ii + i1 + PERM[(jj + j1 + PERM[(kk + k1) as usize]) as usize]) as usize] % 12;
    let gi2 =
        PERM[(ii + i2 + PERM[(jj + j2 + PERM[(kk + k2) as usize]) as usize]) as usize] % 12;
    let gi3 = PERM[(ii + 1 + PERM[(jj + 1 + PERM[(kk + 1) as usize]) as usize]) as usize] % 12;

    let mut t0 = 0.6f32 - x0 * x0 - y0 * y0 - z0 * z0;
    if t0 < 0.0 {
        n0 = 0.0;
    } else {
        t0 *= t0;
        n0 = t0 * t0 * dot3(&GRAD3[gi0 as usize], x0, y0, z0);
    }

    let mut t1 = 0.6f32 - x1 * x1 - y1 * y1 - z1 * z1;
    if t1 < 0.0 {
        n1 = 0.0;
    } else {
        t1 *= t1;
        n1 = t1 * t1 * dot3(&GRAD3[gi1 as usize], x1, y1, z1);
    }

    let mut t2 = 0.6f32 - x2 * x2 - y2 * y2 - z2 * z2;
    if t2 < 0.0 {
        n2 = 0.0;
    } else {
        t2 *= t2;
        n2 = t2 * t2 * dot3(&GRAD3[gi2 as usize], x2, y2, z2);
    }

    let mut t3 = 0.6f32 - x3 * x3 - y3 * y3 - z3 * z3;
    if t3 < 0.0 {
        n3 = 0.0;
    } else {
        t3 *= t3;
        n3 = t3 * t3 * dot3(&GRAD3[gi3 as usize], x3, y3, z3);
    }

    // The result is scaled to stay just inside [-1,1].
    32.0 * (n0 + n1 + n2 + n3)
}

/// 4D raw Simplex noise.
pub fn raw_noise_4d(x: f32, y: f32, z: f32, w: f32) -> f32 {
    // The skewing and unskewing factors are hairy again for the 4D case.
    let f4 = (5.0f32.sqrt() - 1.0) / 4.0;
    let g4 = (5.0f32 - 5.0f32.sqrt()) / 20.0;
    let (n0, n1, n2, n3, n4);

    // Skew the (x,y,z,w) space to determine which cell of 24 simplices we're in.
    let s = (x + y + z + w) * f4;
    let i = fastfloor(x + s);
    let j = fastfloor(y + s);
    let k = fastfloor(z + s);
    let l = fastfloor(w + s);
    let t = (i + j + k + l) as f32 * g4;
    let x0_origin = i as f32 - t;
    let y0_origin = j as f32 - t;
    let z0_origin = k as f32 - t;
    let w0_origin = l as f32 - t;

    let x0 = x - x0_origin;
    let y0 = y - y0_origin;
    let z0 = z - z0_origin;
    let w0 = w - w0_origin;

    // To find out which of the 24 possible simplices we're in we determine the
    // magnitude ordering of x0, y0, z0 and w0: six pair-wise comparisons whose
    // results add up binary bits for an integer index.
    let c1 = if x0 > y0 { 32 } else { 0 };
    let c2 = if x0 > z0 { 16 } else { 0 };
    let c3 = if y0 > z0 { 8 } else { 0 };
    let c4 = if x0 > w0 { 4 } else { 0 };
    let c5 = if y0 > w0 { 2 } else { 0 };
    let c6 = if z0 > w0 { 1 } else { 0 };
    let c: usize = c1 + c2 + c3 + c4 + c5 + c6;

    // simplex[c] is a 4-vector with the numbers 0, 1, 2 and 3 in some order.
    // Many values of c will never occur. We use thresholding to set the
    // coordinates in turn, from the largest magnitude.
    // The number 3 is at the position of the largest coordinate.
    let i1 = i32::from(SIMPLEX[c][0] >= 3);
    let j1 = i32::from(SIMPLEX[c][1] >= 3);
    let k1 = i32::from(SIMPLEX[c][2] >= 3);
    let l1 = i32::from(SIMPLEX[c][3] >= 3);
    // The number 2 is at the second largest coordinate.
    let i2 = i32::from(SIMPLEX[c][0] >= 2);
    let j2 = i32::from(SIMPLEX[c][1] >= 2);
    let k2 = i32::from(SIMPLEX[c][2] >= 2);
    let l2 = i32::from(SIMPLEX[c][3] >= 2);
    // The number 1 is at the second smallest coordinate.
    let i3 = i32::from(SIMPLEX[c][0] >= 1);
    let j3 = i32::from(SIMPLEX[c][1] >= 1);
    let k3 = i32::from(SIMPLEX[c][2] >= 1);
    let l3 = i32::from(SIMPLEX[c][3] >= 1);
    // The fifth corner has all coordinate offsets = 1, so no lookup is needed.

    let x1 = x0 - i1 as f32 + g4;
    let y1 = y0 - j1 as f32 + g4;
    let z1 = z0 - k1 as f32 + g4;
    let w1 = w0 - l1 as f32 + g4;
    let x2 = x0 - i2 as f32 + 2.0 * g4;
    let y2 = y0 - j2 as f32 + 2.0 * g4;
    let z2 = z0 - k2 as f32 + 2.0 * g4;
    let w2 = w0 - l2 as f32 + 2.0 * g4;
    let x3 = x0 - i3 as f32 + 3.0 * g4;
    let y3 = y0 - j3 as f32 + 3.0 * g4;
    let z3 = z0 - k3 as f32 + 3.0 * g4;
    let w3 = w0 - l3 as f32 + 3.0 * g4;
    let x4 = x0 - 1.0 + 4.0 * g4;
    let y4 = y0 - 1.0 + 4.0 * g4;
    let z4 = z0 - 1.0 + 4.0 * g4;
    let w4 = w0 - 1.0 + 4.0 * g4;

    // Work out the hashed gradient indices of the five simplex corners.
    let ii = i & 255;
    let jj = j & 255;
    let kk = k & 255;
    let ll = l & 255;
    let gi0 =
        PERM[(ii + PERM[(jj + PERM[(kk + PERM[ll as usize]) as usize]) as usize]) as usize] % 32;
    let gi1 = PERM[(ii
        + i1
        + PERM[(jj + j1 + PERM[(kk + k1 + PERM[(ll + l1) as usize]) as usize]) as usize])
        as usize]
        % 32;
    let gi2 = PERM[(ii
        + i2
        + PERM[(jj + j2 + PERM[(kk + k2 + PERM[(ll + l2) as usize]) as usize]) as usize])
        as usize]
        % 32;
    let gi3 = PERM[(ii
        + i3
        + PERM[(jj + j3 + PERM[(kk + k3 + PERM[(ll + l3) as usize]) as usize]) as usize])
        as usize]
        % 32;
    let gi4 = PERM[(ii
        + 1
        + PERM[(jj + 1 + PERM[(kk + 1 + PERM[(ll + 1) as usize]) as usize]) as usize])
        as usize]
        % 32;

    let mut t0 = 0.6f32 - x0 * x0 - y0 * y0 - z0 * z0 - w0 * w0;
    if t0 < 0.0 {
        n0 = 0.0;
    } else {
        t0 *= t0;
        n0 = t0 * t0 * dot4(&GRAD4[gi0 as usize], x0, y0, z0, w0);
    }

    let mut t1 = 0.6f32 - x1 * x1 - y1 * y1 - z1 * z1 - w1 * w1;
    if t1 < 0.0 {
        n1 = 0.0;
    } else {
        t1 *= t1;
        n1 = t1 * t1 * dot4(&GRAD4[gi1 as usize], x1, y1, z1, w1);
    }

    let mut t2 = 0.6f32 - x2 * x2 - y2 * y2 - z2 * z2 - w2 * w2;
    if t2 < 0.0 {
        n2 = 0.0;
    } else {
        t2 *= t2;
        n2 = t2 * t2 * dot4(&GRAD4[gi2 as usize], x2, y2, z2, w2);
    }

    let mut t3 = 0.6f32 - x3 * x3 - y3 * y3 - z3 * z3 - w3 * w3;
    if t3 < 0.0 {
        n3 = 0.0;
    } else {
        t3 *= t3;
        n3 = t3 * t3 * dot4(&GRAD4[gi3 as usize], x3, y3, z3, w3);
    }

    let mut t4 = 0.6f32 - x4 * x4 - y4 * y4 - z4 * z4 - w4 * w4;
    if t4 < 0.0 {
        n4 = 0.0;
    } else {
        t4 *= t4;
        n4 = t4 * t4 * dot4(&GRAD4[gi4 as usize], x4, y4, z4, w4);
    }

    // Sum up and scale the result to cover the range [-1,1].
    27.0 * (n0 + n1 + n2 + n3 + n4)
}

pub fn fastfloor(x: f32) -> i32 {
    if x > 0.0 {
        x as i32
    } else {
        x as i32 - 1
    }
}

fn dot2(g: &[i32; 3], x: f32, y: f32) -> f32 {
    g[0] as f32 * x + g[1] as f32 * y
}

fn dot3(g: &[i32; 3], x: f32, y: f32, z: f32) -> f32 {
    g[0] as f32 * x + g[1] as f32 * y + g[2] as f32 * z
}

fn dot4(g: &[i32; 4], x: f32, y: f32, z: f32, w: f32) -> f32 {
    g[0] as f32 * x + g[1] as f32 * y + g[2] as f32 * z + g[3] as f32 * w
}

/// Fill `map` with 4D simplex noise wrapped around a torus.
///
/// `256 / seed` is **integer** division on a possibly negative `i32` — Rust and
/// C++ both truncate towards zero, so this matches. A seed of exactly 0 divides
/// by zero: undefined behaviour in C++, a panic here. The probability of the
/// generator producing it is 2^-32.
pub fn simplexnoise(seed: i32, map: &mut [f32], width: i32, height: i32, persistence: f32) -> i32 {
    let inv_width = 1.0 / width as f32;
    let inv_height = 1.0 / height as f32;
    let noise_scale = 0.593f32;
    let ka = (256 / seed) as f32;
    // int64 intermediates avoid overflow, then mod 256 (negative remainders are
    // possible and intentional — Rust's `%` truncates like C++'s).
    let kb = ((seed as i64 * 567) % 256) as f32;
    let kc = ((seed as i64 * seed as i64) % 256) as f32;
    // `567 - seed` overflows `int` in C++ when seed is very negative (UB that
    // wraps in practice); wrapping keeps the same result.
    let kd = (567i32.wrapping_sub(seed) % 256) as f32;

    for y in 0..height {
        for x in 0..width {
            // The x-offset defines the circle; a full circle is two pi radians.
            let f_nx = x as f32 * inv_width;
            let f_ny = y as f32 * inv_height;
            let f_rdx = f_nx * 2.0 * PI;
            let f_rdy = f_ny * 4.0 * PI;
            let a = f_rdx.sin();
            let b = f_rdx.cos();
            let c = f_rdy.sin();
            let d = f_rdy.cos();
            let v = scaled_octave_noise_4d(
                16.0,
                persistence,
                0.5,
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

    0
}
