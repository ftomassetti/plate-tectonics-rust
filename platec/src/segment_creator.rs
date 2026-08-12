//! Port of `src/segment_creator.hpp` / `src/segment_creator.cpp`.
//!
//! The C++ `MySegmentCreator` class exists purely to bundle references to the
//! bounds, the height map and the segments; here those arrive through
//! [`SegmentCtx`] and the whole thing collapses into a free function.
//!
//! The C++ keeps the span scratch buffers in `static` vectors as a performance
//! cache; [`SpanScratch`] does the same, owned by the `Segments` the fill runs
//! against, so a fill allocates nothing after the first call.

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

/// Reusable span buffers for the flood fill in [`create_segment`].
///
/// Besides holding the `todo`/`done` span lists across calls, this tracks which
/// lines still carry unprocessed spans. The C++ rescans every line of the plate
/// on every round of the fill; walking the active lines instead visits exactly
/// the same lines in exactly the same (ascending) order, just without the empty
/// ones in between.
#[derive(Default)]
pub struct SpanScratch {
    todo: Vec<Vec<u32>>,
    done: Vec<Vec<u32>>,
    /// Lines whose `todo` is non-empty, kept sorted ascending.
    active: Vec<u32>,
    /// Lines whose buffers need emptying once the fill is done.
    touched: Vec<u32>,
    is_touched: Vec<bool>,
}

impl SpanScratch {
    fn prepare(&mut self, height: usize) {
        if self.todo.len() < height {
            self.todo.resize_with(height, Vec::new);
            self.done.resize_with(height, Vec::new);
            self.is_touched.resize(height, false);
        }
    }

    fn mark_touched(&mut self, line: u32) {
        if !self.is_touched[line as usize] {
            self.is_touched[line as usize] = true;
            self.touched.push(line);
        }
    }

    fn push_todo(&mut self, line: u32, start: u32, end: u32) {
        self.todo[line as usize].push(start);
        self.todo[line as usize].push(end);
        if let Err(pos) = self.active.binary_search(&line) {
            self.active.insert(pos, line);
        }
        self.mark_touched(line);
    }

    fn push_done(&mut self, line: u32, start: u32, end: u32) {
        self.done[line as usize].push(start);
        self.done[line as usize].push(end);
        self.mark_touched(line);
    }

    /// The lowest active line strictly greater than `after`, which is what the
    /// C++'s ascending `for line in 0..height` scan lands on next. Spans pushed
    /// to a line above the cursor are therefore picked up in this same round,
    /// and spans pushed below it wait for the next one — as in the original.
    fn next_active(&self, after: i64) -> Option<u32> {
        let from = self.active.partition_point(|&l| (l as i64) <= after);
        self.active.get(from).copied()
    }

    fn deactivate_if_drained(&mut self, line: u32) {
        if self.todo[line as usize].is_empty() {
            if let Ok(pos) = self.active.binary_search(&line) {
                self.active.remove(pos);
            }
        }
    }

    /// Empty every buffer this fill touched, leaving the allocations in place.
    fn finish(&mut self) {
        for &line in self.touched.iter() {
            self.todo[line as usize].clear();
            self.done[line as usize].clear();
            self.is_touched[line as usize] = false;
        }
        self.touched.clear();
        self.active.clear();
    }

    /// Find an unscanned span on this line.
    fn scan_spans(&mut self, line: usize, start: &mut u32, end: &mut u32, bounds_width: u32) {
        loop {
            *end = self.todo[line].pop().unwrap();
            *start = self.todo[line].pop().unwrap();

            // Reduce any done spans from this span. Saved coordinates are AT the
            // point that was included last to the span — that's why equalities
            // matter.
            let mut j = 0;
            while j < self.done[line].len() {
                if *start >= self.done[line][j] && *start <= self.done[line][j + 1] {
                    *start = self.done[line][j + 1] + 1;
                }
                if *end >= self.done[line][j] && *end <= self.done[line][j + 1] {
                    *end = self.done[line][j].wrapping_sub(1);
                }
                j += 2;
            }

            // Unsigned-ness hacking! Required to fix the underflow of end - 1.
            *start |= u32::from(*end >= bounds_width).wrapping_neg();
            *end = end.wrapping_sub(u32::from(*end >= bounds_width));

            if !(*start > *end && !self.todo[line].is_empty()) {
                break;
            }
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

    // Borrowed out of `segments` so that the fill can hold it mutably next to
    // the segment ids; handed back before returning.
    let mut scratch = segments.take_scratch();
    scratch.prepare(bounds_height as usize);

    segments.ids_mut()[origin_index as usize] = id;
    scratch.push_todo(y, x, x);

    loop {
        let mut lines_processed = 0u32;
        let mut cursor: i64 = -1;

        while let Some(line) = scratch.next_active(cursor) {
            cursor = line as i64;
            let line_us = line as usize;

            let mut start = 0u32;
            let mut end = 0u32;
            scratch.scan_spans(line_us, &mut start, &mut end, bounds_width);
            scratch.deactivate_if_drained(line);

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
                scratch.push_todo(line, bounds_width - 1, bounds_width - 1);
            }

            // Check if we should wrap around the right edge.
            if bounds_width == ctx.world_dimension.get_width()
                && end == bounds_width - 1
                && segments.ids()[line_here as usize] > id
                && map[line_here as usize] >= CONT_BASE
            {
                segments.ids_mut()[line_here as usize] = id;
                scratch.push_todo(line, 0, 0);
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

                        scratch.push_todo(row_above, a, b);
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

                        scratch.push_todo(row_below, a, b);
                        j += 1; // Skip the last scanned point.
                    }
                    j += 1;
                }
            }

            scratch.push_done(line, start, end);
            lines_processed += 1;
        }

        if lines_processed == 0 {
            break;
        }
    }

    scratch.finish();
    segments.put_scratch(scratch);
    segments.add(p_data);

    id
}
