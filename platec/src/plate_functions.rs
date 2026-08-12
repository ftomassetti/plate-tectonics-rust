//! Crust sampling helpers shared by the erosion code.

use crate::geometry::WorldDimension;
use crate::heightmap::HeightMap;

/// The neighbouring crust values and their map offsets, as computed by
/// [`calculate_crust`].
#[derive(Clone, Copy, Debug, Default)]
pub struct CrustNeighbours {
    pub w_crust: f32,
    pub e_crust: f32,
    pub n_crust: f32,
    pub s_crust: f32,
    pub w: u32,
    pub e: u32,
    pub n: u32,
    pub s: u32,
}

pub fn calculate_crust(
    x: u32,
    y: u32,
    index: u32,
    world_dimension: &WorldDimension,
    map: &HeightMap,
    width: u32,
    height: u32,
) -> CrustNeighbours {
    // Build masks for accessible directions (4-way). Allow wrapping around map
    // edges if the plate has world-wide dimensions.
    let world_width = world_dimension.get_width();
    let world_height = world_dimension.get_height();
    let width_bit = width == world_width;
    let height_bit = height == world_height;
    let w_mask = u32::from((x > 0) | width_bit).wrapping_neg();
    let e_mask = u32::from((x < width - 1) | width_bit).wrapping_neg();
    let n_mask = u32::from((y > 0) | height_bit).wrapping_neg();
    let s_mask = u32::from((y < height - 1) | height_bit).wrapping_neg();

    // Calculate the x and y offsets of the neighbour directions. If a neighbour
    // is outside the plate edges, set it to zero; this protects map memory
    // reads from segment faulting.
    let x_mod = x % world_width;
    let y_mod = y % world_height;
    let x_mod_minus_1 = if x_mod == 0 { world_width - 1 } else { x_mod - 1 };
    let x_mod_plus_1 = if x_mod + 1 == world_width { 0 } else { x_mod + 1 };
    let y_mod_minus_1 = if y_mod == 0 {
        world_height - 1
    } else {
        y_mod - 1
    };
    let y_mod_plus_1 = if y_mod + 1 == world_height {
        0
    } else {
        y_mod + 1
    };
    let mut w = if w_mask as i32 == -1 { x_mod_minus_1 } else { 0 };
    let mut e = if e_mask as i32 == -1 { x_mod_plus_1 } else { 0 };
    let mut n = if n_mask as i32 == -1 { y_mod_minus_1 } else { 0 };
    let mut s = if s_mask as i32 == -1 { y_mod_plus_1 } else { 0 };

    // Calculate offsets within map memory.
    w = y.wrapping_mul(width).wrapping_add(w);
    e = y.wrapping_mul(width).wrapping_add(e);
    n = n.wrapping_mul(width).wrapping_add(x);
    s = s.wrapping_mul(width).wrapping_add(x);

    // Extract the neighbours' heights, applying validity filtering: 0 is invalid.
    let at = |i: u32| map[i as usize];
    let centre = at(index);
    CrustNeighbours {
        w_crust: at(w) * (w_mask & u32::from(at(w) < centre)) as f32,
        e_crust: at(e) * (e_mask & u32::from(at(e) < centre)) as f32,
        n_crust: at(n) * (n_mask & u32::from(at(n) < centre)) as f32,
        s_crust: at(s) * (s_mask & u32::from(at(s) < centre)) as f32,
        w,
        e,
        n,
        s,
    }
}
