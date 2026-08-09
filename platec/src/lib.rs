//! plate-tectonics — a Rust port of the C++ plate tectonics simulation library
//! (<https://github.com/Mindwerks/plate-tectonics>).
//!
//! Copyright (C) 2012-2013 Lauri Viitanen
//! Copyright (C) 2014-2015 Federico Tomassetti, Bret Curtis
//! Licensed under the GNU LGPL v2.1 or later.
//!
//! The port is deliberately mechanical: unsigned wraparound tricks, `f32`
//! widths and the exact random-number draw order are preserved so that output
//! matches the original.

// The port is mechanical, and several lints fire on constructs kept verbatim
// from the C++ for numerical fidelity (literal precision, `-1.0f * x`,
// negated float comparisons, and so on).
#![allow(clippy::excessive_precision)]
#![allow(clippy::manual_abs_diff)]
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::neg_cmp_op_on_partial_ord)]
#![allow(clippy::neg_multiply)]
#![allow(clippy::should_implement_trait)]

pub mod api;
pub mod bounds;
pub mod geometry;
pub mod heightmap;
pub mod lithosphere;
pub mod mass;
pub mod movement;
pub mod noise;
pub mod plate;
pub mod plate_functions;
pub mod rectangle;
pub mod segment_creator;
pub mod segment_data;
pub mod segments;
pub mod simplerandom;
pub mod simplexnoise;
mod simplexnoise_tables;
pub mod sqrdmd;
pub mod utils;
pub mod world_point;
