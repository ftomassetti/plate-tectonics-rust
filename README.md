# plate-tectonics-rust

A Rust port of the C++ [plate-tectonics](https://github.com/Mindwerks/plate-tectonics)
simulation library, compiled to WebAssembly and driveable from the browser.

The port started out deliberately **mechanical**: unsigned wraparound tricks,
`f32` widths and — most importantly — the exact random-number draw order and
copy points were preserved, so the Rust build reproduced the original's output
bit for bit.

It has since **moved past that**. Three modelling problems in the original are
fixed here, so the output no longer matches the C++ and is not meant to — see
[Divergences from the C++](#divergences-from-the-c) below.

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

`cargo test` runs the full port of the C++ googletest suite — **55 tests**,
matching the original one-for-one, plus the two acceptance cases that are
commented out in the C++ source (ported as `#[ignore]`d).

Running the suite in a **debug** build is deliberate: the original relies on
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
simulation while watching it evolve. Four views are available — hypsometric
terrain (using the colour ramp from the C++ `examples/map_drawing.cpp`),
grayscale height, plate ownership, and crust age — with optional plate-boundary
outlines and per-plate velocity arrows.

A 512×512 world steps in roughly 6 ms, so the simulation renders smoothly at one
iteration per animation frame.

## Divergences from the C++

These are intentional. The C++ behaviour is not available as a build option —
if you need it, use the original library.

* **The base terrain repeated twice down the map.** `createSlowNoise` sweeps `x`
  over 2π across the width but `y` over 4π across the height. Both coordinates
  drive a circle fed into the 4D simplex noise, and one full sweep is what makes
  the result tile — two sweeps make it tile twice, so the bottom half of every
  world was a near-copy of the top half. Measured on the 513×513 grid
  `Lithosphere::new` builds, the vertical self-correlation of the base noise at
  lag 256 was **+0.9999**; with the sweep corrected it is +0.19, on a normal
  decaying autocorrelation curve.
* **Regenerated sea floor came out in hard iso-age bands.** Every cell uncovered
  by a moving plate in one iteration shares an age, and the buoyancy pass turns
  age into height with no noise at all, so each iteration's wake was a
  hard-edged band trailing the plate — the long straight streaks across the deep
  ocean. New crust now gets ±10% height jitter, hashed from `(x, y, iteration)`
  rather than drawn from `randsource` so the random draw order is untouched.
* **Plate overlaps were resolved per pixel.** With near-equal heights the tie
  went to crust age, which varies cell to cell, so the winner alternated and the
  plate map shredded into thin interleaved slivers. The tie band is now sized to
  something physically meaningful and broken by continuity — whoever held the
  cell last iteration keeps it. Connected components of the plate map after 250
  iterations at 512×512 drop from 137 to 82 (seed 12345) and 152 to 62 (seed
  41833258).

Note the plate map records the *topmost* plate per cell, so a contiguous plate
that dips under a neighbour legitimately shows as more than one patch.

## Reproducibility

The regression test (`platec/tests/test_regression.rs`) runs a full 600×400
seed-12345 simulation to completion and compares heightmap statistics against
recorded baselines. It keeps the shape of the C++ `test/test_regression.cpp`,
but the baselines are this implementation's own — it guards our output against
unintended change rather than checking fidelity to the original.

The initial-state baselines are stable across platforms: the map is thresholded
to a pair of constants and the sea-level search pins the land fraction, so the
aggregates barely move even when the terrain does. The final state accumulates
floating-point error and can drift between architectures; the recorded values
come from macOS ARM64.

## Notable design decisions

* **`SimpleRandom` is `Copy`** and passed by value in exactly the places the C++
  passes it by value. `Movement`'s constructor in particular copies the
  generator *before* drawing from the parameter, so `rot_dir` and the initial
  angle consume the *same* random value — `test_movement.rs` pins this.
* **The `plate ↔ Segments ↔ MySegmentCreator ↔ Bounds/HeightMap` pointer cycle**
  in the C++ is replaced by a `SegmentCtx` borrow bundle passed into
  `Segments::get_continent_at`, with segment creation as a free function. This
  is the single largest structural deviation, and it is behaviour-preserving.
* **`ASSERT` logs rather than aborts.** The C++ macro aborts in debug builds but
  only logs in release — and the reference test suite is built in Release, where
  several assertions legitimately fire on paths that then return `BAD_INDEX`.
  Build with `--features strict_asserts` to make them fatal.
* **Buffers are owned `Vec`s.** `Matrix::from_vec` replaces the C++
  pointer-adopting constructor; `HeightMap`/`AgeMap`/`IndexMap` are aliases of
  `Matrix<T>` as in the original.

## License

LGPL 2.1 or later, inherited from the original library. See `LICENSE`.
