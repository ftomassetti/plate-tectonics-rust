//! WebAssembly bindings for the `platec` plate tectonics simulation.
//!
//! The API mirrors the C++ `platecapi` shim: create a simulation, step it one
//! iteration at a time, and read the height/plate maps between steps. That
//! step-wise shape is what lets the browser demo render the simulation as it
//! progresses.

// The port is mechanical, and several lints fire on constructs kept verbatim
// from the C++ for numerical fidelity (literal precision, `-1.0f * x`,
// negated float comparisons, and so on).
#![allow(clippy::excessive_precision)]
#![allow(clippy::manual_abs_diff)]
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::neg_cmp_op_on_partial_ord)]
#![allow(clippy::neg_multiply)]
#![allow(clippy::should_implement_trait)]

use platec::api::Simulation as CoreSimulation;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct Simulation {
    inner: CoreSimulation,
}

#[wasm_bindgen]
impl Simulation {
    /// Create a simulation. Mirrors `platec_api_create`.
    #[wasm_bindgen(constructor)]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
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
    ) -> Result<Simulation, JsError> {
        let inner = CoreSimulation::create(
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
        )
        .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Simulation { inner })
    }

    /// Advance the simulation by one iteration.
    pub fn step(&mut self) {
        self.inner.step();
    }

    #[wasm_bindgen(js_name = isFinished)]
    pub fn is_finished(&self) -> bool {
        self.inner.is_finished()
    }

    pub fn width(&self) -> u32 {
        self.inner.width()
    }

    pub fn height(&self) -> u32 {
        self.inner.height()
    }

    #[wasm_bindgen(js_name = plateCount)]
    pub fn plate_count(&self) -> u32 {
        self.inner.plate_count()
    }

    #[wasm_bindgen(js_name = iterationCount)]
    pub fn iteration_count(&self) -> u32 {
        self.inner.iteration_count()
    }

    #[wasm_bindgen(js_name = cycleCount)]
    pub fn cycle_count(&self) -> u32 {
        self.inner.cycle_count()
    }

    #[wasm_bindgen(js_name = plateVelocityX)]
    pub fn plate_velocity_x(&self, index: u32) -> f32 {
        self.inner.velocity_unit_vector(index).0
    }

    #[wasm_bindgen(js_name = plateVelocityY)]
    pub fn plate_velocity_y(&self, index: u32) -> f32 {
        self.inner.velocity_unit_vector(index).1
    }

    // -- Zero-copy buffer access ------------------------------------------
    //
    // JS builds `new Float32Array(wasm.memory.buffer, ptr, width * height)`.
    // Such a view is invalidated whenever wasm memory grows, and `step()` does
    // allocate, so the view must be rebuilt after **every** step.

    #[wasm_bindgen(js_name = heightmapPtr)]
    pub fn heightmap_ptr(&self) -> *const f32 {
        self.inner.heightmap().as_ptr()
    }

    #[wasm_bindgen(js_name = platesmapPtr)]
    pub fn platesmap_ptr(&self) -> *const u32 {
        self.inner.platesmap().as_ptr()
    }

    #[wasm_bindgen(js_name = agemapPtr)]
    pub fn agemap_ptr(&self) -> *const u32 {
        self.inner.agemap().as_ptr()
    }

    // -- Copying accessors -------------------------------------------------
    //
    // Safe against memory growth and suitable for `postMessage` from a Worker.

    pub fn heightmap(&self) -> Vec<f32> {
        self.inner.heightmap().to_vec()
    }

    pub fn platesmap(&self) -> Vec<u32> {
        self.inner.platesmap().to_vec()
    }

    pub fn agemap(&self) -> Vec<u32> {
        self.inner.agemap().to_vec()
    }
}
