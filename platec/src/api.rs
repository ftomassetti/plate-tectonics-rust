//! The public simulation API.
//!
//! [`Simulation`] is an owning handle around a [`Lithosphere`].

use crate::geometry::WorldDimension;
use crate::lithosphere::{Lithosphere, PlatecError};
use crate::movement::MovementLike;
use crate::plate::Plate;

pub struct Simulation {
    litho: Lithosphere,
}

impl Simulation {
    /// Create a simulation.
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

    /// Simulate one iteration.
    pub fn step(&mut self) {
        self.litho.update();
    }

    /// Whether the simulation has run to completion.
    pub fn is_finished(&self) -> bool {
        self.litho.is_finished()
    }

    /// The height of each cell of the world.
    pub fn heightmap(&self) -> &[f32] {
        self.litho.get_topography()
    }

    /// The index of the plate owning each cell of the world.
    pub fn platesmap(&self) -> &[u32] {
        self.litho.get_plates_map()
    }

    /// The creation time of the crust in each cell of the world.
    pub fn agemap(&self) -> &[u32] {
        self.litho.get_age_map()
    }

    /// Width of the world in cells.
    pub fn width(&self) -> u32 {
        self.litho.get_width()
    }

    /// Height of the world in cells.
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

    /// The unit velocity vector of the given plate.
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
