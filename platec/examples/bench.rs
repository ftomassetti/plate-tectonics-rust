//! Time the plate simulation at a couple of sizes, to compare native against wasm.
use platec::api::Simulation;
use std::time::Instant;

fn main() {
    for (w, h, plates) in [(512u32, 512u32, 10u32), (512, 256, 10), (256, 128, 10)] {
        let t0 = Instant::now();
        let mut sim = Simulation::create(28070, w, h, 0.65, 60, 0.02, 1_000_000, 0.33, 2, plates).unwrap();
        let mut steps = 0;
        while !sim.is_finished() {
            sim.step();
            steps += 1;
        }
        let el = t0.elapsed();
        println!(
            "{w}x{h} {plates}p: {steps} steps in {:.2}s ({:.2} ms/step)",
            el.as_secs_f64(),
            el.as_secs_f64() * 1000.0 / steps as f64
        );
    }
}
