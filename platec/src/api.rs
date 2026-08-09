//! Safe replacement for `src/platecapi.hpp` / `src/platecapi.cpp`.
//!
//! The C++ API is a `void*`-based C shim over `lithosphere`, plus a global
//! registry of live lithospheres that exists only to support one legacy
//! id-based age-map lookup. None of that is needed here, so [`Simulation`] is
//! simply an owning handle with the same operations.

use crate::geometry::WorldDimension;
use crate::lithosphere::{Lithosphere, PlatecError};
use crate::movement::MovementLike;
use crate::plate::Plate;

pub struct Simulation {
    litho: Lithosphere,
}

impl Simulation {
    /// Port of `platec_api_create`.
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        seed: u32,
        width: u32,
        height: u32,
        sea_level: f32,
        erosion_period: u32,
        folding_ratio: f32,
        aggr_overlap_abs: u32,
        aggr_overlap_rel: f32,
        cycle_count: u32,
        num_plates: u32,
    ) -> Result<Self, PlatecError> {
        Ok(Self {
            litho: Lithosphere::new(
                seed,
                width,
                height,
                sea_level,
                erosion_period,
                folding_ratio,
                aggr_overlap_abs,
                aggr_overlap_rel,
                cycle_count,
                num_plates,
            )?,
        })
    }

    /// Port of `platec_api_step`: simulate one iteration.
    pub fn step(&mut self) {
        self.litho.update();
    }

    /// Port of `platec_api_is_finished`.
    pub fn is_finished(&self) -> bool {
        self.litho.is_finished()
    }

    /// Port of `platec_api_get_heightmap`.
    pub fn heightmap(&self) -> &[f32] {
        self.litho.get_topography()
    }

    /// Port of `platec_api_get_platesmap`.
    pub fn platesmap(&self) -> &[u32] {
        self.litho.get_plates_map()
    }

    /// Port of `platec_api_get_agemap`.
    pub fn agemap(&self) -> &[u32] {
        self.litho.get_age_map()
    }

    /// Port of `lithosphere_getMapWidth`.
    pub fn width(&self) -> u32 {
        self.litho.get_width()
    }

    /// Port of `lithosphere_getMapHeight`.
    pub fn height(&self) -> u32 {
        self.litho.get_height()
    }

    pub fn plate_count(&self) -> u32 {
        self.litho.get_plate_count()
    }

    pub fn iteration_count(&self) -> u32 {
        self.litho.get_iteration_count()
    }

    pub fn cycle_count(&self) -> u32 {
        self.litho.get_cycle_count()
    }

    pub fn world_dimension(&self) -> &WorldDimension {
        self.litho.get_world_dimension()
    }

    pub fn plate(&self, index: u32) -> &Plate {
        self.litho.get_plate(index)
    }

    /// Ports `platec_api_velocity_unity_vector_x` / `_y`.
    pub fn velocity_unit_vector(&self, plate_index: u32) -> (f32, f32) {
        let v = self.litho.get_plate(plate_index).velocity_unit_vector();
        (v.x(), v.y())
    }

    pub fn lithosphere(&self) -> &Lithosphere {
        &self.litho
    }

    pub fn lithosphere_mut(&mut self) -> &mut Lithosphere {
        &mut self.litho
    }
}
