//! Small end-to-end smoke run: build a world and step it to completion.
use platec::api::Simulation;

fn main() {
    let mut sim = Simulation::create(3, 64, 64, 0.65, 60, 0.02, 1_000_000, 0.33, 2, 10).unwrap();
    let mut steps = 0;
    while !sim.is_finished() {
        sim.step();
        steps += 1;
        if steps > 5000 {
            panic!("simulation did not finish");
        }
    }
    let h = sim.heightmap();
    let (min, max) = h.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
    println!("finished after {steps} steps; cycles={} min={min} max={max}", sim.cycle_count());
}
