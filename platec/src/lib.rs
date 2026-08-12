//! plate-tectonics — a plate tectonics simulation.
//!
//! Copyright (C) 2012-2013 Lauri Viitanen
//! Copyright (C) 2014-2015 Bret Curtis
//! Copyright (C) 2014-2026 Federico Tomassetti
//! Licensed under the GNU LGPL v2.1 or later.
//!
//! Plates are grown from seed points on a toroidal world, then moved, collided,
//! subducted, eroded and aggregated over a few hundred iterations to build a
//! heightmap. Unsigned wraparound and `f32` widths are load-bearing throughout.

// Several lints fire on constructs kept deliberately for numerical fidelity
// (literal precision, `-1.0 * x`, negated float comparisons, and so on).
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
