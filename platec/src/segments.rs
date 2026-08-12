//! The continents within a plate: the continent id of each cell of
//! continental crust, plus the per-continent data.
//!
//! The state that continent lookup needs from its owning plate — the bounds
//! and the height map — is passed in explicitly as a [`SegmentCtx`] rather than
//! held as back-pointers, which would make the ownership cyclic.

use crate::bounds::Bounds;
use crate::geometry::WorldDimension;
use crate::heightmap::HeightMap;
use crate::platec_assert;
use crate::segment_creator::{create_segment, SpanScratch};
use crate::segment_data::{SegmentData, SegmentDataApi};

pub type ContinentId = u32;

/// The plate-owned state that segment creation reads.
pub struct SegmentCtx<'a> {
    pub bounds: &'a Bounds,
    pub map: &'a HeightMap,
    pub world_dimension: &'a WorldDimension,
}

/// A plate holds one of these behind a `Box` so that tests can inject mocks.
pub trait SegmentsApi {
    fn area(&self) -> u32;
    fn reset(&mut self);
    fn reassign(&mut self, newarea: u32, tmps: Vec<u32>);
    fn shift(&mut self, d_lft: u32, d_top: u32);
    fn size(&self) -> u32;
    fn seg(&self, index: u32) -> &dyn SegmentDataApi;
    fn seg_mut(&mut self, index: u32) -> &mut dyn SegmentDataApi;
    fn add(&mut self, data: SegmentData);
    /// Continent at the given world index.
    fn id(&self, index: u32) -> ContinentId;
    fn set_id(&mut self, index: u32, id: ContinentId);
    fn get_continent_at(&mut self, x: u32, y: u32, ctx: &SegmentCtx) -> ContinentId;
}

pub struct Segments {
    /// Details of each crust segment.
    seg_data: Vec<SegmentData>,
    /// Segment ID of each piece of continental crust.
    segment: Vec<ContinentId>,
    /// Should be the same as the bounds area of the plate.
    area: u32,
    /// Span buffers reused by `create_segment`, so that a fill allocates
    /// nothing after the first call.
    scratch: SpanScratch,
}

impl Segments {
    pub fn new(plate_area: u32) -> Self {
        Self {
            seg_data: Vec::new(),
            // No cell belongs to a continent yet.
            segment: vec![u32::MAX; plate_area as usize],
            area: plate_area,
            scratch: SpanScratch::default(),
        }
    }

    /// Move the span scratch out so that `create_segment` can hold it mutably
    /// alongside `&mut Segments`. Always paired with [`Segments::put_scratch`].
    pub(crate) fn take_scratch(&mut self) -> SpanScratch {
        std::mem::take(&mut self.scratch)
    }

    pub(crate) fn put_scratch(&mut self, scratch: SpanScratch) {
        self.scratch = scratch;
    }

    pub fn ids(&self) -> &[ContinentId] {
        &self.segment
    }

    pub fn ids_mut(&mut self) -> &mut [ContinentId] {
        &mut self.segment
    }

    pub fn data(&self) -> &[SegmentData] {
        &self.seg_data
    }

    pub fn data_mut(&mut self) -> &mut [SegmentData] {
        &mut self.seg_data
    }
}

impl SegmentsApi for Segments {
    fn area(&self) -> u32 {
        self.area
    }

    fn reset(&mut self) {
        for id in self.segment.iter_mut() {
            *id = u32::MAX;
        }
        self.seg_data.clear();
    }

    fn reassign(&mut self, newarea: u32, tmps: Vec<u32>) {
        self.area = newarea;
        self.segment = tmps;
    }

    fn shift(&mut self, d_lft: u32, d_top: u32) {
        for s in self.seg_data.iter_mut() {
            s.shift(d_lft, d_top);
        }
    }

    fn size(&self) -> u32 {
        self.seg_data.len() as u32
    }

    fn seg(&self, index: u32) -> &dyn SegmentDataApi {
        platec_assert!((index as usize) < self.seg_data.len(), "Invalid index");
        &self.seg_data[index as usize]
    }

    fn seg_mut(&mut self, index: u32) -> &mut dyn SegmentDataApi {
        platec_assert!((index as usize) < self.seg_data.len(), "Invalid index");
        &mut self.seg_data[index as usize]
    }

    fn add(&mut self, data: SegmentData) {
        self.seg_data.push(data);
    }

    fn id(&self, index: u32) -> ContinentId {
        self.segment[index as usize]
    }

    fn set_id(&mut self, index: u32, id: ContinentId) {
        self.segment[index as usize] = id;
    }

    fn get_continent_at(&mut self, x: u32, y: u32, ctx: &SegmentCtx) -> ContinentId {
        let mut lx = x;
        let mut ly = y;
        let index = ctx.bounds.get_valid_map_index(&mut lx, &mut ly);
        let mut seg = self.id(index);

        if seg >= self.size() {
            // The segments act as a cache: this computes something that
            // would have to be computed anyway.
            seg = create_segment(lx, ly, ctx, self);
        }

        platec_assert!(seg < self.size(), "Could not create segment");
        seg
    }
}
