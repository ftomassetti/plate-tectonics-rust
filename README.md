# plate-tectonics-rust

A plate tectonics simulation in Rust, compiled to WebAssembly and driveable
from the browser.

Plates are grown from seed points on a toroidal world, then moved, collided,
subducted, eroded and aggregated over a few hundred iterations to build a
heightmap. See [Simulation notes](#simulation-notes) for the parts of the model
worth knowing about.

The project has been going since 2012 and has changed language twice along the
way — see [History](#history).

## Layout

```
platec/          core library crate (no dependencies, pure std)
platec-wasm/     wasm-bindgen bindings exposing a step-wise API
www/             browser demo (plain HTML + ES modules, no framework)
PLAN.md          the porting plan this implementation follows
```

## Building and testing

The toolchain comes from Homebrew's `rustup`, whose shims are not on the default
PATH. Either add them permanently:

```sh
echo 'export PATH="/opt/homebrew/opt/rustup/bin:$PATH"' >> ~/.zshrc
```

…or prefix the commands below with `PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.

```sh
cargo test                 # debug: overflow checks on (slow: ~1 min for the regression test)
cargo test --release       # release: the same 55 tests in ~5 s
cargo clippy --all-targets
```

The `parallel` feature (on by default) spreads the per-plate work and the
regeneration sweep across cores with rayon. It is bit-identical to the serial
path, so results do not depend on it or on the core count. `platec-wasm` turns
it off, since wasm has no threads by default; build the library with
`--no-default-features` to do the same.

Measured on 12 cores, whole-run average:

| world | serial | parallel |
|-------|--------|----------|
| 512x512   |  2.92 ms/step |  2.50 ms/step |
| 1024x1024 | 10.68 ms/step |  8.21 ms/step |
| 2048x2048 | 40.39 ms/step | 29.63 ms/step |

Most of a step is not parallelisable as the simulation stands: building the
height and plate-index maps is ~60% of the time and is order-dependent across
plates, with plates mutating each other's crust as they go.

`cargo test` runs **55 tests**, plus two `#[ignore]`d platform-dependent
acceptance cases.

Running the suite in a **debug** build is deliberate: the simulation relies on
unsigned integer overflow in roughly two dozen places, and Rust's debug overflow
checks panic on any site where that was not modelled explicitly with
`wrapping_*`.

## Building the browser demo

```sh
wasm-pack build platec-wasm --target web --out-dir ../www/pkg --release
python3 -m http.server 8000 -d www
# then open http://localhost:8000
```

The demo lets you configure every parameter of `platec_api_create` (seed,
dimensions, sea level, plate count, erosion period, folding ratio, aggregation
thresholds, cycles), generate a world, and then run / pause / single-step the
simulation while watching it evolve. Five views are available — hypsometric
terrain, grayscale height, plate ownership, crust age, and a lit 3D terrain mesh
— with optional plate-boundary outlines and per-plate velocity arrows on the 2D
maps.

The **3D view** (`www/view3d.js`) needs WebGL2 and falls back to the 2D terrain
if it is missing. Drag to orbit, scroll to zoom, or leave it auto-rotating. The
mesh is a static grid uploaded once; only the height texture changes per frame,
and the vertex shader displaces the grid by sampling it, so nothing is
recomputed on the CPU as the simulation runs. Two knobs matter:

* **Relief** scales the vertical exaggeration. Terrain above the 90th percentile
  is compressed logarithmically first — folding produces isolated cells many
  times taller than the ridges around them, and left alone they render as
  needles that dominate the silhouette.
* **Colours** picks the hypsometric ramp, shared with the 2D terrain view.
  *Natural* is desaturated enough that shading reads on top of it and adds a
  snow line; *Hypsometric* is the demo's original high-saturation ramp.

A 512×512 world steps in roughly 6 ms, so the simulation renders smoothly at one
iteration per animation frame.

## Simulation notes

Three parts of the model are worth calling out, because each fixes a problem
that is easy to reintroduce.

* **The base terrain must tile exactly once per axis.** `create_slow_noise`
  sweeps both `x` and `y` over 2π, each driving a circle fed into the 4D simplex
  noise. One full sweep per axis is what makes the result tile; two sweeps on an
  axis make it tile *twice*, which shows up as the bottom half of every world
  being a near-copy of the top half. Measured on the 513×513 grid
  `Lithosphere::new` builds, the vertical self-correlation of the base noise at
  lag 256 is +0.19, on a normal decaying autocorrelation curve; with a doubled
  y sweep it is **+0.9999**.
* **Regenerated sea floor gets height jitter.** Every cell uncovered by a moving
  plate in one iteration shares an age, and the buoyancy pass turns age into
  height. Without noise, each iteration's wake is a hard-edged iso-height band
  trailing the plate — long straight streaks across the deep ocean. New crust
  gets ±10% jitter, hashed from `(x, y, iteration)` rather than drawn from
  `randsource`, so the random draw order is untouched.
* **Plate overlaps resolve per region, not per pixel.** Where two overlapping
  plates have near-equal height, breaking the tie by crust age shreds the plate
  map into thin interleaved slivers, because age varies cell to cell. The tie
  band is sized to the height noise and broken by continuity instead: whoever
  held the cell last iteration keeps it. That takes the connected components of
  the plate map after 250 iterations at 512×512 from 137 to 82 (seed 12345) and
  152 to 62 (seed 41833258).

Note the plate map records the *topmost* plate per cell, so a contiguous plate
that dips under a neighbour legitimately shows as more than one patch.

### Plate reorganisation

A cycle ends by discarding the plates and re-partitioning the world. Done
naively this shreds continents: the old code seeded plates uniformly at random
and grew them isotropically, so a new boundary was as likely to run down the
middle of a craton as along a coastline, and the halves were then pulled apart.
The damage was concentrated in the ~30 iterations after each restart — one seed
went from a single continent holding 92% of all land to the largest holding 50%
in 28 steps, where the preceding 430 iterations had cost it almost nothing.

Four changes address it, and `examples/render.rs` measures the result:

```sh
cargo run --release --example render -p platec -- /tmp/worlds 12345 777 40001
```

* **Plates grow around continents, not through them.** `grow_plates` is a
  multi-source Dijkstra where entering continental crust costs about ten times
  what ocean does, and more again with thickness. Fronts race around landmasses
  and meet in the ocean, so a continent is usually claimed whole by whichever
  plate reaches it first.
* **Seeds prefer ocean.** Selection redraws up to 8 times looking for oceanic
  crust, because rifts nucleate in thin, weak lithosphere.
* **Cratons are not cut.** Continental crust older than 200 iterations costs 200
  to cross, so a front goes the long way round.
* **Cycles end on assembly, not on a clock.** Once the largest landmass has held
  90% of all land for 150 iterations, the supercontinent has settled and rifts
  apart. Previously the dominant trigger was a bare 600-iteration cap; the one
  condition with a geological reading never fired at all.
* **Plate geometry carries over.** Each plate seeds the next cycle from one of
  its own oceanic cells, and only two plates are reseeded fresh.

Mean over eight seeds at 512×512, run to completion:

| stage | landmasses | largest landmass | land |
|-------|-----------|------------------|------|
| before | 5.6 | 64.2% | 23.5% |
| + cost-weighted growth, ocean seeds | 6.4 | 77.8% | 22.6% |
| + cratons | 5.5 | 72.5% | 22.1% |
| + Wilson cycle | 4.9 | 74.0% | 22.6% |
| + carried-over geometry | 4.5 | 76.4% | 21.1% |

Eight seeds is the minimum worth trusting here. Individual commit messages quote
three-seed figures measured as each change went in, and several of those do not
survive the wider sample — cratons look like a clear coherence win at three
seeds and roughly neutral at eight, and carried-over geometry looks like a clear
regression at three and a modest gain at eight. The table above is the one to
believe.

## Reproducibility

The regression test (`platec/tests/test_regression.rs`) runs a full 600×400
seed-12345 simulation to completion and compares heightmap statistics against
recorded baselines, guarding against unintended change.

The initial-state baselines are stable across platforms: the map is thresholded
to a pair of constants and the sea-level search pins the land fraction, so the
aggregates barely move even when the terrain does. The final state accumulates
floating-point error and can drift between architectures; the recorded values
come from macOS ARM64.

## Notable design decisions

* **`SimpleRandom` is `Copy`** and deliberately passed by value in places where
  the caller's generator must not be advanced. `Movement`'s constructor in
  particular copies the generator *before* drawing from the parameter, so
  `rot_dir` and the initial angle consume the *same* random value —
  `test_movement.rs` pins this.
* **Segment creation takes a `SegmentCtx`.** The bounds and height map it needs
  are passed in explicitly rather than held as back-pointers from `Segments`,
  which would make the ownership cyclic.
* **`platec_assert!` logs rather than aborts.** Several assertions legitimately
  fire on paths that then return `BAD_INDEX`. Build with
  `--features strict_asserts` to make them fatal.
* **Buffers are owned `Vec`s.** `HeightMap`, `AgeMap` and `IndexMap` are all
  aliases of `Matrix<T>`.

## History

**2012–2013 — `platec`, by Lauri Viitanen.** The simulation started as part of
an academic thesis at Metropolia University of Applied Sciences in Helsinki.
Everything the model does — growing plates from seed points, the buoyancy rules
that decide which plate subducts, folding crust at collisions, the restart
cycle — comes from that work.

**2014 onwards — C++, by Federico Tomassetti.** Federico picked the project up
and translated it to C++, published as
[Mindwerks/plate-tectonics](https://github.com/Mindwerks/plate-tectonics), with
contributions from Bret Curtis. That version became the terrain engine behind
[WorldEngine](https://github.com/Mindwerks/worldengine).

**2026 — Rust.** Translated again, this time to Rust, and compiled to
WebAssembly so it runs in a browser. The translation was initially mechanical
and reproduced the C++ output bit for bit, which is how it was checked. It has
since stopped tracking the C++ version: see [Simulation notes](#simulation-notes)
for the modelling changes, none of which exist upstream.

## License

LGPL 2.1 or later, inherited from the original library. See `LICENSE`.
