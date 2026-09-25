# Browser accelerator prototype

Research owner: [amemory#8](https://github.com/netkeep80/amemory/issues/8).

This experiment proves only the browser execution path:

```text
Rust -> WebAssembly -> browser
browser JS -> WebGPU -> GPU/iGPU compute -> readback
```

It is **not** an MTS/A-memory semantic implementation yet.

## What the prototype proves

A successful run must show:

```text
Rust/WASM = LOADED
CPU control = PASS
WebGPU = WEBGPU_AVAILABLE
GPU compute = PASS [2, 3, 4, 5]
```

If WebGPU is unavailable, the page must say so explicitly. It must not silently pretend that the CPU/WASM fallback was GPU execution.

## Toolchain

Pinned first-spike Rust toolchain:

```text
Rust 1.98.1
target: wasm32-unknown-unknown
```

There are intentionally no Rust crate dependencies in the first witness.

## Build locally

From this directory:

```bash
rustup toolchain install 1.98.1 --profile minimal
rustup target add --toolchain 1.98.1 wasm32-unknown-unknown

cargo +1.98.1 build \
  --manifest-path rust/Cargo.toml \
  --target wasm32-unknown-unknown \
  --release

cp rust/target/wasm32-unknown-unknown/release/amemory_browser_probe.wasm \
  web/amemory_browser_probe.wasm
```

Serve the directory over localhost:

```bash
python -m http.server 8000 --directory web
```

Open:

```text
http://localhost:8000/
```

Localhost is suitable for development; the public demo is intended to run over HTTPS on GitHub Pages.

## GitHub Pages

The repository workflow builds the Rust WASM artifact and deploys this demo as a Pages artifact.

Expected project-site URL:

```text
https://netkeep80.github.io/amemory/
```

The repository may require this one-time setting:

```text
Settings -> Pages -> Build and deployment -> Source -> GitHub Actions
```

All browser assets use relative paths so the project-site subpath works.

## Why the first spike uses JS WebGPU directly

The first witness deliberately separates concerns:

- Rust proves native code can become browser WASM;
- raw WASM exports prove the core can be called without framework glue;
- direct JavaScript WebGPU proves accelerator dispatch/readback independently;
- a later experiment can compare this split with Rust + `wgpu` on the web target.

That prevents a larger abstraction stack from hiding whether a failure belongs to Rust/WASM, WebGPU, shader execution, or deployment.
