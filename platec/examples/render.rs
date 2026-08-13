//! Render finished worlds to PPM, and report how fragmented they are.
//!
//! Used to compare the effect of changes to the plate model: run it before and
//! after, and diff the images and the numbers.
//!
//! ```text
//! cargo run --release --example render -p platec -- <out-dir> [seed...]
//! ```

use platec::api::Simulation;
use std::collections::VecDeque;

const LAND: f32 = 1.0; // CONTINENTAL_BASE

/// Hypsometric ramp, matching the browser demo's "Natural" palette. Each band
/// interpolates `from` -> `to` up to its quantile of the height distribution.
const BANDS: [(f32, [f32; 3], [f32; 3]); 7] = [
    (0.15, [6.0, 32.0, 60.0], [10.0, 47.0, 82.0]),
    (0.70, [10.0, 47.0, 82.0], [29.0, 95.0, 138.0]),
    (0.75, [29.0, 95.0, 138.0], [134.0, 201.0, 208.0]),
    (0.90, [79.0, 122.0, 58.0], [143.0, 154.0, 78.0]),
    (0.95, [143.0, 154.0, 78.0], [169.0, 128.0, 63.0]),
    (0.99, [169.0, 128.0, 63.0], [107.0, 74.0, 51.0]),
    (1.00, [107.0, 74.0, 51.0], [236.0, 231.0, 226.0]),
];

fn quantiles(h: &[f32]) -> [f32; 7] {
    let mut sorted = h.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut out = [0.0f32; 7];
    for (i, (q, _, _)) in BANDS.iter().enumerate() {
        let idx = ((*q as f64) * (sorted.len() - 1) as f64) as usize;
        out[i] = sorted[idx];
    }
    out
}

/// Connected landmasses on the toroidal world.
///
/// Returns the number bigger than 0.1% of the world, the share of all land in
/// the largest one, and the share of the world that is land.
fn landmasses(h: &[f32], w: usize, ht: usize) -> (usize, f64, f64) {
    let mut seen = vec![false; w * ht];
    let mut sizes = vec![];
    let mut q = VecDeque::new();
    let land = h.iter().filter(|&&v| v >= LAND).count();

    for start in 0..w * ht {
        if seen[start] || h[start] < LAND {
            continue;
        }
        seen[start] = true;
        q.push_back(start);
        let mut n = 0usize;
        while let Some(c) = q.pop_front() {
            n += 1;
            let (x, y) = (c % w, c / w);
            for (nx, ny) in [
                ((x + 1) % w, y),
                ((x + w - 1) % w, y),
                (x, (y + 1) % ht),
                (x, (y + ht - 1) % ht),
            ] {
                let m = ny * w + nx;
                if !seen[m] && h[m] >= LAND {
                    seen[m] = true;
                    q.push_back(m);
                }
            }
        }
        sizes.push(n);
    }

    sizes.sort_unstable_by(|a, b| b.cmp(a));
    let significant = sizes
        .iter()
        .filter(|&&s| s as f64 > 0.001 * (w * ht) as f64)
        .count();
    (
        significant,
        sizes.first().copied().unwrap_or(0) as f64 / land.max(1) as f64,
        land as f64 / (w * ht) as f64,
    )
}

fn colour(v: f32, qs: &[f32; 7], min: f32) -> [f32; 3] {
    let mut lo = min;
    for (i, (_, from, to)) in BANDS.iter().enumerate() {
        if v < qs[i] || i == 6 {
            let d = qs[i] - lo;
            let t = if d > 0.0 {
                ((v - lo) / d).clamp(0.0, 1.0)
            } else {
                0.0
            };
            return [
                from[0] + (to[0] - from[0]) * t,
                from[1] + (to[1] - from[1]) * t,
                from[2] + (to[2] - from[2]) * t,
            ];
        }
        lo = qs[i];
    }
    unreachable!()
}

/// Colour by height, then shade by slope so the relief reads.
fn render(h: &[f32], w: usize, ht: usize) -> Vec<u8> {
    let qs = quantiles(h);
    let min = h.iter().copied().fold(f32::INFINITY, f32::min);
    let mut px = vec![0u8; w * ht * 3];

    for y in 0..ht {
        for x in 0..w {
            let i = y * w + x;
            let c = colour(h[i], &qs, min);

            // Central differences on the toroidal map, lit from the north-west.
            let l = h[y * w + (x + w - 1) % w];
            let r = h[y * w + (x + 1) % w];
            let u = h[((y + ht - 1) % ht) * w + x];
            let d = h[((y + 1) % ht) * w + x];
            let (nx, nz) = ((l - r) * 3.0, (u - d) * 3.0);
            let len = (nx * nx + nz * nz + 1.0).sqrt();
            let lambert = ((-0.55 * nx + 0.72 + 0.42 * nz) / len).clamp(0.0, 1.0);
            let shade = 0.45 + 0.75 * lambert;

            for k in 0..3 {
                px[i * 3 + k] = (c[k] * shade).clamp(0.0, 255.0) as u8;
            }
        }
    }
    px
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().expect("usage: render <out-dir> [seed...]");
    let seeds: Vec<u32> = {
        let rest: Vec<u32> = args.filter_map(|a| a.parse().ok()).collect();
        if rest.is_empty() {
            vec![12345, 777, 40001]
        } else {
            rest
        }
    };

    let (w, h) = (512usize, 512usize);
    std::fs::create_dir_all(&dir).expect("could not create output directory");

    let mut total_masses = 0.0;
    let mut total_largest = 0.0;
    let mut total_land = 0.0;

    for seed in &seeds {
        let mut sim =
            Simulation::create(*seed, w as u32, h as u32, 0.65, 60, 0.02, 1_000_000, 0.33, 2, 10)
                .unwrap();
        while !sim.is_finished() {
            sim.step();
        }

        let hm = sim.heightmap();
        let (masses, largest, land) = landmasses(hm, w, h);
        total_masses += masses as f64;
        total_largest += largest;
        total_land += land;
        println!(
            "seed {seed:6}: {masses:2} landmasses, largest holds {:5.1}% of land, land = {:4.1}% of world",
            largest * 100.0,
            land * 100.0
        );

        let px = render(hm, w, h);
        let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
        out.extend_from_slice(&px);
        std::fs::write(format!("{dir}/seed{seed}.ppm"), out).expect("could not write image");
    }

    let n = seeds.len() as f64;
    println!(
        "MEAN over {} seeds: {:.1} landmasses, largest {:.1}%, land {:.1}%",
        seeds.len(),
        total_masses / n,
        total_largest * 100.0 / n,
        total_land * 100.0 / n
    );
}
