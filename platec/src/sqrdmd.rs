//! Fractal height maps via the square-diamond algorithm.
//!
//! Author of the original: Lauri Viitanen, 2011-08-09.
//!
//! The order in which `next_float_signed()` is drawn is what makes the output
//! reproducible, so the loop structure is preserved exactly.

use crate::platec_assert;
use crate::simplerandom::SimpleRandom;

/// Scale the values of the map between [0, 1[.
pub fn normalize(arr: &mut [f32]) {
    if arr.is_empty() {
        return;
    }
    let mut min = arr[0];
    let mut max = arr[0];

    for &v in arr.iter().skip(1) {
        min = if min < v { min } else { v };
        max = if max > v { max } else { v };
    }

    let diff = max - min;

    if diff > 0.0 {
        for v in arr.iter_mut() {
            *v = (*v - min) / diff;
        }
    }
}

/// Store `sum` at `a` iff the truncated value there is zero.
#[inline]
fn save_sum(map: &mut [f32], a: i32, sum: f32) {
    let is_zero = map[a as usize] as i32 == 0;
    if is_zero {
        map[a as usize] = sum;
    }
}

/// Generate a two dimensional fractal height map.
///
/// Values other than zero in the target array are left unmodified. `rgh`
/// controls the roughness: 0.0 produces a completely flat map, 1.0 a
/// completely random one.
///
/// * `size` — length of the map's side: 2^x + 1
///
/// Returns zero on success.
pub fn sqrdmd(seed: u32, map: &mut [f32], size: i32, rgh: f32) -> i32 {
    let mut randsource = SimpleRandom::new(seed);

    let full_size = size * size;

    let mut temp = size - 1;
    // MUST EQUAL 2^x + 1!
    platec_assert!(
        !(temp & (temp - 1) != 0 || temp & 3 != 0),
        "Side should be 2**n +1"
    );
    let mut slope = rgh;
    let mut step = size & !1;

    // Calculate midpoint ("diamond step").
    let mut dy = step * size;
    let mut sum = (map[0] + map[step as usize] + map[dy as usize] + map[(dy + step) as usize])
        * 0.25
        + slope * randsource.next_float_signed();
    save_sum(map, 0, sum);
    let center_sum = sum;

    // Calculate each sub diamonds' center points ("square step").
    // Top row.
    let mut p0 = step >> 1;
    sum = (map[0] + map[step as usize] + center_sum + center_sum) * 0.25
        + slope * randsource.next_float_signed();
    save_sum(map, p0, sum);
    // Left column.
    let mut p1 = p0 * size;
    sum = (map[0] + map[dy as usize] + center_sum + center_sum) * 0.25
        + slope * randsource.next_float_signed();
    save_sum(map, p1, sum);
    // Copy top value into bottom row.
    map[(full_size + p0 - size) as usize] = map[p0 as usize];
    // Copy left value into right column.
    map[(p1 + size - 1) as usize] = map[p1 as usize];
    slope *= rgh;
    step >>= 1;

    // Enter the main loop.
    while step > 1 {
        // Calculate the midpoint of sub squares on the map ("diamond step").
        let dx = step;
        dy = step * size;
        let mut i = (step >> 1) * (size + 1);
        let line_jump = step * size + 1 + step - size;

        let mut y0 = 0;
        let mut y1 = dy;
        while y1 < size * size {
            let mut x0 = 0;
            let mut x1 = dx;
            while x1 < size {
                sum = (map[(y0 + x0) as usize]
                    + map[(y0 + x1) as usize]
                    + map[(y1 + x0) as usize]
                    + map[(y1 + x1) as usize])
                    * 0.25
                    + slope * randsource.next_float_signed();
                let masked = i32::from(map[i as usize] as i32 == 0);
                map[i as usize] =
                    map[i as usize] * (1 - masked) as f32 + sum * masked as f32;
                x0 += dx;
                x1 += dx;
                i += step;
            }
            // There's an additional step taken at the end of the last valid
            // loop. That step isn't valid because the row ends right then, so
            // remove it manually here so that 'i' points again to the index
            // accessed last.
            i += line_jump - step;
            y0 += dy;
            y1 += dy;
        }

        // Calculate each sub diamond's center point ("square step"). The
        // diamond gets its left and right vertices from the square corners of
        // the last iteration and its top and bottom vertices from the "diamond
        // step" just performed.
        i = step >> 1;
        p0 = step; // right
        p1 = i * size + i; // bottom
        let mut p2 = 0; // left
        let mut p3 = full_size + i - (i + 1) * size; // top (wrapping edges)

        // Calculate "diamond" values for the top row in the map.
        while p0 < size {
            sum = (map[p0 as usize] + map[p1 as usize] + map[p2 as usize] + map[p3 as usize])
                * 0.25
                + slope * randsource.next_float_signed();
            let masked = i32::from(map[i as usize] as i32 == 0);
            map[i as usize] = map[i as usize] * (1 - masked) as f32 + sum * masked as f32;
            // Copy it into the bottom row.
            map[(full_size + i - size) as usize] = map[i as usize];
            p0 += step;
            p1 += step;
            p2 += step;
            p3 += step;
            i += step;
        }

        // Starting from 'y = step >> 1' saves recalculating the same things
        // twice and guarantees that data will not be read beyond the top row of
        // the map. 'size - (step >> 1)' guarantees that data will not be read
        // beyond the bottom row.
        let mut y = step >> 1;
        temp = 0;
        while y < size - (step >> 1) {
            p0 = step >> 1; // right
            p1 = p0 * size; // bottom
            p2 = -p0; // left
            p3 = -p1; // top
            // For even rows add step/2, otherwise add nothing.
            let mut x = p0 * temp; // Init 'x' while it's easy.
            i = x;
            i += y * size; // Move 'i' into the correct row.
            p0 += i;
            p1 += i;
            // For odd rows p2 (left) wraps around the map edges.
            p2 += i + (size - 1) * i32::from(temp == 0);
            p3 += i;
            // size - (step >> 1) guarantees that data will not be read beyond
            // the rightmost column of the map.
            while x < size - (step >> 1) {
                sum = (map[p0 as usize]
                    + map[p1 as usize]
                    + map[p2 as usize]
                    + map[p3 as usize])
                    * 0.25
                    + slope * randsource.next_float_signed();
                let masked = i32::from(map[i as usize] as i32 == 0);
                map[i as usize] =
                    map[i as usize] * (1 - masked) as f32 + sum * masked as f32;
                p0 += step;
                p1 += step;
                p2 += step;
                p3 += step;
                i += step;
                // If we start from the leftmost column the left point (p2) goes
                // over the right border, so wrap it around into the beginning
                // of the previous row's left line.
                p2 -= (size - 1) * i32::from(x == 0);
                x += step;
            }
            // Copy the row's first element into its last.
            i = y * size;
            map[(i + size - 1) as usize] = map[i as usize];

            y += step >> 1;
            temp = i32::from(temp == 0);
        }
        slope *= rgh; // Reduce the amount of randomness for the next round.
        step >>= 1; // Split squares and diamonds in half.
    }

    0
}
