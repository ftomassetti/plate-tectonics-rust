//! Per-continent bookkeeping: area, collision count and bounding box.

use crate::rectangle::Rectangle;

pub trait SegmentDataAccess {
    fn get_left(&self) -> u32;
    fn get_right(&self) -> u32;
    fn get_top(&self) -> u32;
    fn get_bottom(&self) -> u32;
    fn is_empty(&self) -> bool;
    fn area(&self) -> u32;
    fn coll_count(&self) -> u32;
}

pub trait SegmentDataApi: SegmentDataAccess {
    fn inc_coll_count(&mut self);
    fn inc_area(&mut self);
    fn enlarge_to_contain(&mut self, x: u32, y: u32);
    fn mark_non_existent(&mut self);
    fn shift(&mut self, dx: u32, dy: u32);
}

/// Container for details about a segmented crust area on this plate.
#[derive(Clone, Copy, Debug)]
pub struct SegmentData {
    rectangle: Rectangle,
    /// Number of locations this area consists of.
    area: u32,
    /// Number of collisions on this segment.
    coll_count: u32,
}

impl SegmentData {
    pub fn new(rectangle: Rectangle, area: u32) -> Self {
        Self {
            rectangle,
            area,
            coll_count: 0,
        }
    }

    pub fn set_left(&mut self, v: u32) {
        self.rectangle.set_left(v);
    }
    pub fn set_right(&mut self, v: u32) {
        self.rectangle.set_right(v);
    }
    pub fn set_top(&mut self, v: u32) {
        self.rectangle.set_top(v);
    }
    pub fn set_bottom(&mut self, v: u32) {
        self.rectangle.set_bottom(v);
    }

    pub fn inc_area_by(&mut self, amount: u32) {
        self.area = self.area.wrapping_add(amount);
    }
}

impl SegmentDataAccess for SegmentData {
    fn get_left(&self) -> u32 {
        self.rectangle.get_left()
    }
    fn get_right(&self) -> u32 {
        self.rectangle.get_right()
    }
    fn get_top(&self) -> u32 {
        self.rectangle.get_top()
    }
    fn get_bottom(&self) -> u32 {
        self.rectangle.get_bottom()
    }
    fn is_empty(&self) -> bool {
        self.area == 0
    }
    fn area(&self) -> u32 {
        self.area
    }
    fn coll_count(&self) -> u32 {
        self.coll_count
    }
}

impl SegmentDataApi for SegmentData {
    fn inc_coll_count(&mut self) {
        self.coll_count = self.coll_count.wrapping_add(1);
    }
    fn inc_area(&mut self) {
        self.area = self.area.wrapping_add(1);
    }
    fn enlarge_to_contain(&mut self, x: u32, y: u32) {
        self.rectangle.enlarge_to_contain(x, y);
    }
    fn mark_non_existent(&mut self) {
        self.area = 0;
    }
    fn shift(&mut self, dx: u32, dy: u32) {
        self.rectangle.shift(dx, dy);
    }
}
