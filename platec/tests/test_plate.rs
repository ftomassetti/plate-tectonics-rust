//! Tests for a single tectonic plate.
//! cases from that file live in `test_noise.rs`).

// Several lints fire on constructs kept deliberately for numerical fidelity
// (literal precision, `-1.0 * x`, negated float comparisons, and so on).
#![allow(clippy::excessive_precision)]
#![allow(clippy::manual_abs_diff)]
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::neg_cmp_op_on_partial_ord)]
#![allow(clippy::neg_multiply)]
#![allow(clippy::should_implement_trait)]

#[macro_use]
mod common;

use std::sync::{Arc, Mutex};

/// A `Cell` that is also `Send`, so a mock can live in the plate's
/// `Box<dyn SegmentsApi + Send>`. Same `get`/`set` shape as `Cell`.
#[derive(Debug, Default)]
struct SharedCell<T: Copy>(Mutex<T>);

impl<T: Copy> SharedCell<T> {
    fn new(value: T) -> Self {
        Self(Mutex::new(value))
    }
    fn get(&self) -> T {
        *self.0.lock().unwrap()
    }
    fn set(&self, value: T) {
        *self.0.lock().unwrap() = value;
    }
}

use platec::geometry::WorldDimension;
use platec::mass::MassLike;
use platec::noise::create_noise;
use platec::plate::Plate;
use platec::segment_data::{SegmentData, SegmentDataAccess, SegmentDataApi};
use platec::segments::{ContinentId, SegmentCtx, SegmentsApi};
use platec::simplerandom::SimpleRandom;

/// Fill a height map with noise.
fn initialize_heightmap_with_noise(seed: u32, heightmap: &mut [f32], wd: &WorldDimension) {
    create_noise(heightmap, wd, SimpleRandom::new(seed), true);
    for v in heightmap.iter_mut().take(wd.get_area() as usize) {
        if *v < 0.0 {
            *v *= -1.0;
        }
    }
}

fn noisy_heightmap(seed: u32, wd: &WorldDimension) -> Vec<f32> {
    let mut heightmap = vec![0.0f32; wd.get_area() as usize];
    initialize_heightmap_with_noise(seed, &mut heightmap, wd);
    heightmap
}

// ---------------------------------------------------------------------------
// Mocks. `unimplemented!()` marks the methods a given test does not exercise.
// Fields the tests inspect after the mock has been moved into the plate are
// held behind `Arc<SharedCell<_>>` so the test keeps a handle on them.
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct MockSegmentData {
    coll_count: Arc<SharedCell<u32>>,
    area: Arc<SharedCell<u32>>,
    enlarge_point: Arc<SharedCell<Option<(u32, u32)>>>,
}

impl MockSegmentData {
    fn new(coll_count: u32, area: u32) -> Self {
        Self {
            coll_count: Arc::new(SharedCell::new(coll_count)),
            area: Arc::new(SharedCell::new(area)),
            enlarge_point: Arc::new(SharedCell::new(None)),
        }
    }
}

impl SegmentDataAccess for MockSegmentData {
    fn get_left(&self) -> u32 {
        unimplemented!()
    }
    fn get_right(&self) -> u32 {
        unimplemented!()
    }
    fn get_top(&self) -> u32 {
        unimplemented!()
    }
    fn get_bottom(&self) -> u32 {
        unimplemented!()
    }
    fn is_empty(&self) -> bool {
        unimplemented!()
    }
    fn area(&self) -> u32 {
        self.area.get()
    }
    fn coll_count(&self) -> u32 {
        self.coll_count.get()
    }
}

impl SegmentDataApi for MockSegmentData {
    fn inc_coll_count(&mut self) {
        self.coll_count.set(self.coll_count.get() + 1);
    }
    fn inc_area(&mut self) {
        self.area.set(self.area.get() + 1);
    }
    fn enlarge_to_contain(&mut self, x: u32, y: u32) {
        self.enlarge_point.set(Some((x, y)));
    }
    fn mark_non_existent(&mut self) {
        unimplemented!()
    }
    fn shift(&mut self, _dx: u32, _dy: u32) {
        unimplemented!()
    }
}

/// A stand-in for a plate's segments.
struct MockSegments {
    p: (u32, u32),
    id: Arc<SharedCell<ContinentId>>,
    data: MockSegmentData,
}

impl SegmentsApi for MockSegments {
    fn area(&self) -> u32 {
        unimplemented!()
    }
    fn reset(&mut self) {
        unimplemented!()
    }
    fn reassign(&mut self, _newarea: u32, _tmps: Vec<u32>) {
        unimplemented!()
    }
    fn shift(&mut self, _d_lft: u32, _d_top: u32) {
        unimplemented!()
    }
    fn size(&self) -> u32 {
        unimplemented!("(MockSegments::size) Not implemented")
    }
    fn seg(&self, index: u32) -> &dyn SegmentDataApi {
        assert_eq!(index, self.id.get(), "(MockSegments::seg) Unexpected call");
        &self.data
    }
    fn seg_mut(&mut self, index: u32) -> &mut dyn SegmentDataApi {
        assert_eq!(
            index,
            self.id.get(),
            "(MockSegments::seg_mut) Unexpected call"
        );
        &mut self.data
    }
    fn add(&mut self, _data: SegmentData) {
        unimplemented!()
    }
    fn id(&self, _index: u32) -> ContinentId {
        unimplemented!("(MockSegments::id) Not implemented")
    }
    fn set_id(&mut self, _index: u32, _id: ContinentId) {
        unimplemented!()
    }
    fn get_continent_at(&mut self, x: u32, y: u32, _ctx: &SegmentCtx) -> ContinentId {
        assert_eq!(
            (x, y),
            self.p,
            "(MockSegments::getContinentAt) Unexpected call"
        );
        self.id.get()
    }
}

/// A stand-in that records the calls made to it.
struct MockSegments2 {
    p: (u32, u32),
    id: Arc<SharedCell<ContinentId>>,
    data: MockSegmentData,
    index: u32,
}

impl SegmentsApi for MockSegments2 {
    fn area(&self) -> u32 {
        unimplemented!("(MockSegments2::area) Not implemented")
    }
    fn reset(&mut self) {
        unimplemented!("(MockSegments2::reset) Not implemented")
    }
    fn reassign(&mut self, _newarea: u32, _tmps: Vec<u32>) {
        unimplemented!("(MockSegments2::reassign) Not implemented")
    }
    fn shift(&mut self, _d_lft: u32, _d_top: u32) {
        unimplemented!("(MockSegments2::shift) Not implemented")
    }
    fn size(&self) -> u32 {
        unimplemented!("(MockSegments2::size) Not implemented")
    }
    fn seg(&self, index: u32) -> &dyn SegmentDataApi {
        assert_eq!(
            index,
            self.id.get(),
            "(MockSegments2::seg) Unexpected call with id {index}"
        );
        &self.data
    }
    fn seg_mut(&mut self, index: u32) -> &mut dyn SegmentDataApi {
        assert_eq!(
            index,
            self.id.get(),
            "(MockSegments2::seg_mut) Unexpected call with id {index}"
        );
        &mut self.data
    }
    fn add(&mut self, _data: SegmentData) {
        unimplemented!("(MockSegments2::add) Not implemented")
    }
    fn id(&self, index: u32) -> ContinentId {
        assert_eq!(
            index, self.index,
            "(MockSegments2::id) Unexpected value {index}, expected was {}",
            self.index
        );
        self.id.get()
    }
    fn set_id(&mut self, index: u32, id: ContinentId) {
        assert_eq!(
            index, self.index,
            "(MockSegments2::setId) Unexpected call with index {index}"
        );
        self.id.set(id);
    }
    fn get_continent_at(&mut self, x: u32, y: u32, _ctx: &SegmentCtx) -> ContinentId {
        assert_eq!(
            (x, y),
            self.p,
            "(MockSegments2::getContinentAt) Unexpected call"
        );
        self.id.get()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn create_plate_square_does_not_explode() {
    let wd = WorldDimension::new(200, 200);
    let heightmap = noisy_heightmap(678, &wd);
    let _p = Plate::new(123, heightmap, 100, 3, 50, 23, 18, wd);
}

#[test]
fn create_plate_not_square_does_not_explode() {
    let wd = WorldDimension::new(200, 400);
    let heightmap = noisy_heightmap(678, &wd);
    let _p = Plate::new(123, heightmap, 100, 3, 50, 23, 18, wd);
}

#[test]
fn plate_calculate_crust() {
    let wd = WorldDimension::new(256, 128);
    let heightmap = noisy_heightmap(678, &wd);
    let p = Plate::new(123, heightmap, 100, 3, 50, 23, 18, wd);

    // top left corner
    let c = p.calculate_crust(0, 0, 1);
    assert_eq!(0, c.w);
    assert_eq!(1, c.e);
    assert_eq!(0, c.n);
    assert_eq!(100, c.s);

    // bottom right corner
    let c = p.calculate_crust(99, 2, 1);
    assert_eq!(298, c.w);
    assert_eq!(200, c.e);
    assert_eq!(199, c.n);
    assert_eq!(99, c.s);

    // point in the middle
    let c = p.calculate_crust(50, 1, 1);
    assert_eq!(149, c.w);
    assert_eq!(151, c.e);
    assert_eq!(50, c.n);
    assert_eq!(250, c.s);
}

#[test]
fn plate_add_collision() {
    let wd = WorldDimension::new(256, 128);
    let heightmap = noisy_heightmap(678, &wd);
    let mut p = Plate::new(123, heightmap, 100, 3, 50, 23, 18, wd);

    let m_seg = MockSegmentData::new(7, 789);
    let coll_count = m_seg.coll_count.clone();
    let m_segments = MockSegments {
        p: (123, 78),
        id: Arc::new(SharedCell::new(99)),
        data: m_seg,
    };
    p.inject_segments(Box::new(m_segments));

    let area = p.add_collision(123, 78);

    assert_eq!(789, area);
    assert_eq!(8, coll_count.get());
}

#[test]
fn plate_add_crust_by_collision() {
    let world_width = 256u32;
    let world_height = 128u32;
    let wd = WorldDimension::new(world_width, world_height);
    let heightmap = noisy_heightmap(1, &wd);

    // Suppose the plate starts at 170, 70 and ends at 250, 125.
    let mut p = Plate::new(123, heightmap, 80, 55, 170, 70, 18, wd);
    // The point of collision in world coordinates.
    let world_point_x = 240u32;
    let world_point_y = 120u32;
    // The point of collision in plate coordinates.
    let plate_point_x = 70u32;
    let plate_point_y = 50u32;
    let index_in_plate = plate_point_y * 80 + plate_point_x;

    let m_seg = MockSegmentData::new(7, 789);
    let area = m_seg.area.clone();
    let enlarge_point = m_seg.enlarge_point.clone();
    let id = Arc::new(SharedCell::new(99u32));
    let m_segments = MockSegments2 {
        p: (world_point_x, world_point_y),
        id: id.clone(),
        data: m_seg,
        index: index_in_plate,
    };
    p.inject_segments(Box::new(m_segments));

    let timestamp_before = p.get_crust_timestamp(world_point_x, world_point_y);
    let crust_before = p.get_crust(world_point_x, world_point_y);

    // Assumption: the point is within the plate bounds.
    p.add_crust_by_collision(
        world_point_x,
        world_point_y, // Point of impact
        0.8,           // Amount of crust
        123,           // Current age
        99,            // Active continent
    );

    // The age of the point should be updated.
    let timestamp_after = p.get_crust_timestamp(world_point_x, world_point_y);
    assert!(timestamp_after > timestamp_before);
    assert!(timestamp_after < 123);

    // Crust should be increased.
    let crust_after = p.get_crust(world_point_x, world_point_y);
    expect_float_eq!(crust_before + 0.8, crust_after);

    // The active continent should now own the point.
    assert_eq!(99, id.get());

    // The active continent should contain the point.
    let point = enlarge_point.get().expect("enlarge_to_contain was not called");
    assert_eq!(70, point.0);
    assert_eq!(50, point.1);

    // The active continent's area should be increased.
    assert_eq!(790, area.get());
}

#[test]
fn plate_add_crust_by_subduction() {
    let world_width = 256u32;
    let world_height = 128u32;
    let wd = WorldDimension::new(world_width, world_height);
    let heightmap = noisy_heightmap(1, &wd);

    // Suppose the plate starts at 170, 70 and ends at 250, 125.
    let mut p = Plate::new(123, heightmap, 80, 55, 170, 70, 18, wd);
    let world_point_x = 240u32;
    let world_point_y = 120u32;
    let plate_point_x = 70u32;
    let plate_point_y = 50u32;
    let index_in_plate = plate_point_y * 80 + plate_point_x;

    let m_seg = MockSegmentData::new(7, 789);
    let m_segments = MockSegments2 {
        p: (world_point_x, world_point_y),
        id: Arc::new(SharedCell::new(99)),
        data: m_seg,
        index: index_in_plate,
    };
    p.inject_segments(Box::new(m_segments));

    let timestamp_before = p.get_crust_timestamp(world_point_x, world_point_y);
    let crust_before = p.get_crust(world_point_x, world_point_y);
    let mass_before = p.get_mass();

    // Assumption: the point is within the plate bounds.
    let dx = 0.0f32;
    let dy = 0.0f32;
    p.add_crust_by_subduction(
        world_point_x,
        world_point_y, // Point of impact
        0.8,           // Amount of crust
        123,           // Current age
        dx,
        dy, // Direction of the subducting plate
    );

    // Crust should be increased.
    let crust_after = p.get_crust(world_point_x, world_point_y);
    expect_float_eq!(crust_before + 0.8, crust_after);

    // The mass should be increased.
    let mass_after = p.get_mass();
    assert_eq!(mass_before + 0.8, mass_after);

    // The age of the point should be updated.
    let timestamp_after = p.get_crust_timestamp(world_point_x, world_point_y);
    assert!(timestamp_after > timestamp_before);
    assert!(timestamp_after < 123);
}
