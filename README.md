# plate-tectonics-rust

A Rust port of the C++ [plate-tectonics](https://github.com/Mindwerks/plate-tectonics)
simulation library, compiled to WebAssembly and driveable from the browser.

The port is deliberately **mechanical**: unsigned wraparound tricks, `f32`
widths and — most importantly — the exact random-number draw order and copy
points are preserved, so the Rust build can reproduce the original's output.

The default build then **fixes three modelling problems** the original has (see
[Divergences from the C++](#divergences-from-the-c) below). Build with
`--features classic_cpp` to turn those off and get bit-exact C++ behaviour.

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
cargo test --release       # release: the same tests in ~5 s
cargo test --release --features platec/classic_cpp   # adds the C++-baseline regression test
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

All three are behind the `classic_cpp` feature — off by default, so the default
build takes the fix; enable it to get the original behaviour back.

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
the baselines recorded in the C++ `test/test_regression.cpp`. It only applies to
the bit-exact path, so it is compiled only under `classic_cpp`:

```sh
cargo test --release --features platec/classic_cpp
```

There the Rust output matches the **x86-64** baseline to 6+ significant digits:

| metric  | Rust       | C++ x86-64 baseline |
|---------|------------|---------------------|
| min     | 0.04239162 | 0.0423916           |
| max     | 17.840475  | 17.8405             |
| mean    | 0.6240615  | 0.62406             |
| median  | 0.11457818 | 0.114578            |
| std_dev | 0.94567454 | 0.945673            |

It matches x86-64 rather than the ARM64 baseline even when built on Apple
silicon because Clang contracts `a*b+c` into a fused multiply-add on ARM by
default, while Rust never contracts — so the Rust build follows the
non-contracted (MSVC/GCC) arithmetic on every target.

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
