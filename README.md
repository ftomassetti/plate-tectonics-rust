# plate-tectonics-rust

A Rust port of the C++ [plate-tectonics](https://github.com/Mindwerks/plate-tectonics)
simulation library, compiled to WebAssembly and driveable from the browser.

The port is deliberately **mechanical**: unsigned wraparound tricks, `f32`
widths and — most importantly — the exact random-number draw order and copy
points are preserved, so the Rust build reproduces the original's output.

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

## Reproducibility

The regression test (`platec/tests/test_regression.rs`) runs a full 600×400
seed-12345 simulation to completion and compares heightmap statistics against
the baselines recorded in the C++ `test/test_regression.cpp`. The Rust output
matches the **x86-64** baseline to 6+ significant digits:

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
