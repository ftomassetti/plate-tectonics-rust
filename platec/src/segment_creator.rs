//! Port of `src/segment_creator.hpp` / `src/segment_creator.cpp`.
//!
//! The C++ `MySegmentCreator` class exists purely to bundle references to the
//! bounds, the height map and the segments; here those arrive through
//! [`SegmentCtx`] and the whole thing collapses into a free function.
//!
//! The C++ keeps the span scratch buffers in `static` vectors as a performance
//! cache (they are cleared after every call, so this is semantically identical
//! to allocating them locally).

use crate::movement::CONT_BASE;
use crate::rectangle::Rectangle;
use crate::segment_data::{SegmentData, SegmentDataAccess, SegmentDataApi};
use crate::segments::{ContinentId, SegmentCtx, Segments, SegmentsApi};

fn calc_direction(
    x: u32,
    y: u32,
    origin_index: u32,
    id: u32,
    ctx: &SegmentCtx,
    segments: &Segments,
) -> u32 {
    let map = ctx.map;
    let width = ctx.bounds.width();
    let height = ctx.bounds.height();
    let oi = origin_index as usize;

    let can_go_left = x > 0 && map[oi - 1] >= CONT_BASE;
    let can_go_right = x < width - 1 && map[oi + 1] >= CONT_BASE;
    let can_go_up = y > 0 && map[oi - width as usize] >= CONT_BASE;
    let can_go_down = y < height - 1 && map[oi + width as usize] >= CONT_BASE;
    let mut nbour_id = id;

    // This point belongs to no segment yet. However it might be a neighbour to
    // some segment created earlier; if such a neighbour is found, associate
    // this point with it.
    if can_go_left && segments.ids()[oi - 1] < id {
        nbour_id = segments.ids()[oi - 1];
    } else if can_go_right && segments.ids()[oi + 1] < id {
        nbour_id = segments.ids()[oi + 1];
    } else if can_go_up && segments.ids()[oi - width as usize] < id {
        nbour_id = segments.ids()[oi - width as usize];
    } else if can_go_down && segments.ids()[oi + width as usize] < id {
        nbour_id = segments.ids()[oi + width as usize];
    }

    nbour_id
}

/// Find an unscanned span on this line.
fn scan_spans(
    line: usize,
    start: &mut u32,
    end: &mut u32,
    spans_todo: &mut [Vec<u32>],
    spans_done: &[Vec<u32>],
    bounds_width: u32,
) {
    loop {
        *end = spans_todo[line].pop().unwrap();
        *start = spans_todo[line].pop().unwrap();

        // Reduce any done spans from this span. Saved coordinates are AT the
        // point that was included last to the span — that's why equalities
        // matter.
        let mut j = 0;
        while j < spans_done[line].len() {
            if *start >= spans_done[line][j] && *start <= spans_done[line][j + 1] {
                *start = spans_done[line][j + 1] + 1;
            }
            if *end >= spans_done[line][j] && *end <= spans_done[line][j + 1] {
                *end = spans_done[line][j].wrapping_sub(1);
            }
            j += 2;
        }

        // Unsigned-ness hacking! Required to fix the underflow of end - 1.
        *start |= u32::from(*end >= bounds_width).wrapping_neg();
        *end = end.wrapping_sub(u32::from(*end >= bounds_width));

        if !(*start > *end && !spans_todo[line].is_empty()) {
            break;
        }
    }
}

/// Separate a continent at (X, Y) into its own partition.
///
/// The method analyses the pixels 4-ways adjacent at the given location and
/// labels all connected continental points with the same segment ID.
///
/// * `x`, `y` — offsets on the local height map.
///
/// Returns the ID of the created segment.
pub fn create_segment(x: u32, y: u32, ctx: &SegmentCtx, segments: &mut Segments) -> ContinentId {
    let bounds_width = ctx.bounds.width();
    let bounds_height = ctx.bounds.height();
    let origin_index = ctx.bounds.index(x, y);
    let id = segments.size();
    let map = ctx.map;

    if segments.ids()[origin_index as usize] < id {
        return segments.ids()[origin_index as usize];
    }

    let nbour_id = calc_direction(x, y, origin_index, id, ctx, segments);

    if nbour_id < id {
        segments.ids_mut()[origin_index as usize] = nbour_id;
        segments.data_mut()[nbour_id as usize].inc_area();
        segments.data_mut()[nbour_id as usize].enlarge_to_contain(x, y);
        return nbour_id;
    }

    let rect = Rectangle::new(*ctx.world_dimension, x, x, y, y);
    let mut p_data = SegmentData::new(rect, 0);
    let mut spans_todo: Vec<Vec<u32>> = vec![Vec::new(); bounds_height as usize];
    let mut spans_done: Vec<Vec<u32>> = vec![Vec::new(); bounds_height as usize];

    segments.ids_mut()[origin_index as usize] = id;
    spans_todo[y as usize].push(x);
    spans_todo[y as usize].push(x);

    loop {
        let mut lines_processed = 0u32;
        for line in 0..bounds_height {
            let line_us = line as usize;
            if spans_todo[line_us].is_empty() {
                continue;
            }

            let mut start = 0u32;
            let mut end = 0u32;
            scan_spans(
                line_us,
                &mut start,
                &mut end,
                &mut spans_todo,
                &spans_done,
                bounds_width,
            );

            if start > end {
                continue; // Nothing to do here anymore...
            }

            // Calculate line indices, allowing wrapping around map edges.
            let row_above = ((line.wrapping_sub(1)) & u32::from(line > 0).wrapping_neg())
                | ((bounds_height - 1) & u32::from(line == 0).wrapping_neg());
            let row_below =
                (line.wrapping_add(1)) & u32::from(line < bounds_height - 1).wrapping_neg();
            let line_here = line * bounds_width;
            let line_above = row_above * bounds_width;
            let line_below = row_below * bounds_width;

            // Extend the beginning of the line.
            while start > 0
                && segments.ids()[(line_here + start - 1) as usize] > id
                && map[(line_here + start - 1) as usize] >= CONT_BASE
            {
                start -= 1;
                segments.ids_mut()[(line_here + start) as usize] = id;
            }

            // Extend the end of the line.
            while end < bounds_width - 1
                && segments.ids()[(line_here + end + 1) as usize] > id
                && map[(line_here + end + 1) as usize] >= CONT_BASE
            {
                end += 1;
                segments.ids_mut()[(line_here + end) as usize] = id;
            }

            // Check if we should wrap around the left edge.
            if bounds_width == ctx.world_dimension.get_width()
                && start == 0
                && segments.ids()[(line_here + bounds_width - 1) as usize] > id
                && map[(line_here + bounds_width - 1) as usize] >= CONT_BASE
            {
                segments.ids_mut()[(line_here + bounds_width - 1) as usize] = id;
                spans_todo[line_us].push(bounds_width - 1);
                spans_todo[line_us].push(bounds_width - 1);
            }

            // Check if we should wrap around the right edge.
            if bounds_width == ctx.world_dimension.get_width()
                && end == bounds_width - 1
                && segments.ids()[line_here as usize] > id
                && map[line_here as usize] >= CONT_BASE
            {
                segments.ids_mut()[line_here as usize] = id;
                spans_todo[line_us].push(0);
                spans_todo[line_us].push(0);
            }

            // Update the segment area counter.
            p_data.inc_area_by(1 + end - start);

            // Record any changes in extreme dimensions.
            if line < p_data.get_top() {
                p_data.set_top(line);
            }
            if line > p_data.get_bottom() {
                p_data.set_bottom(line);
            }
            if start < p_data.get_left() {
                p_data.set_left(start);
            }
            if end > p_data.get_right() {
                p_data.set_right(end);
            }

            if line > 0 || bounds_height == ctx.world_dimension.get_height() {
                let mut j = start;
                while j <= end {
                    if segments.ids()[(line_above + j) as usize] > id
                        && map[(line_above + j) as usize] >= CONT_BASE
                    {
                        let a = j;
                        segments.ids_mut()[(line_above + a) as usize] = id;

                        j += 1;
                        while j < bounds_width
                            && segments.ids()[(line_above + j) as usize] > id
                            && map[(line_above + j) as usize] >= CONT_BASE
                        {
                            segments.ids_mut()[(line_above + j) as usize] = id;
                            j += 1;
                        }

                        j -= 1; // Last point is invalid.
                        let b = j;

                        spans_todo[row_above as usize].push(a);
                        spans_todo[row_above as usize].push(b);
                        j += 1; // Skip the last scanned point.
                    }
                    j += 1;
                }
            }

            if line < bounds_height - 1 || bounds_height == ctx.world_dimension.get_height() {
                let mut j = start;
                while j <= end {
                    if segments.ids()[(line_below + j) as usize] > id
                        && map[(line_below + j) as usize] >= CONT_BASE
                    {
                        let a = j;
                        segments.ids_mut()[(line_below + a) as usize] = id;

                        j += 1;
                        while j < bounds_width
                            && segments.ids()[(line_below + j) as usize] > id
                            && map[(line_below + j) as usize] >= CONT_BASE
                        {
                            segments.ids_mut()[(line_below + j) as usize] = id;
                            j += 1;
                        }

                        j -= 1; // Last point is invalid.
                        let b = j;

                        spans_todo[row_below as usize].push(a);
                        spans_todo[row_below as usize].push(b);
                        j += 1; // Skip the last scanned point.
                    }
                    j += 1;
                }
            }

            spans_done[line_us].push(start);
            spans_done[line_us].push(end);
            lines_processed += 1;
        }

        if lines_processed == 0 {
            break;
        }
    }

    segments.add(p_data);

    id
}
