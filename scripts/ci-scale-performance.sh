#!/usr/bin/env bash
set -euo pipefail

RUST_TOOLCHAIN="${AMEMORY_RUST_TOOLCHAIN:-1.98.1}"
A_CIRCUIT_MANIFEST="experiments/a-circuit/Cargo.toml"
OPTIMIZED_MANIFEST="experiments/optimized-cpu-rust/Cargo.toml"
TMP_ROOT="$(mktemp -d)"
trap 'rm -rf "${TMP_ROOT}"' EXIT

cargo_pinned() {
  cargo "+${RUST_TOOLCHAIN}" "$@"
}

echo "=== BUILD SCALE RUNNER ==="
cargo_pinned build \
  --release \
  --manifest-path "${OPTIMIZED_MANIFEST}" \
  --bin scale_ladder

echo "=== STORAGE SCALE LADDER ==="
for n in 1000 10000 100000 1000000 10000000; do
  echo "=== STORAGE ${n} ==="
  cargo_pinned run \
    --quiet \
    --release \
    --manifest-path "${OPTIMIZED_MANIFEST}" \
    --bin scale_ladder -- storage "${n}" 200000
done

echo "=== A-CIRCUIT STRUCTURAL PERFORMANCE PROFILES ==="
ripple_log="${TMP_ROOT}/ripple-perf.log"
cargo_pinned test \
  --release \
  --manifest-path "${A_CIRCUIT_MANIFEST}" \
  m3_perf_ripple_width_baseline \
  -- --ignored --nocapture --test-threads=1 2>&1 | tee "${ripple_log}"

for witness in \
  "RIPPLE_PERF width=4 " \
  "RIPPLE_PERF width=8 " \
  "RIPPLE_PERF width=16 " \
  "RIPPLE_PERF width=32 " \
  "RIPPLE_PERF_NOTE=informational-only-no-performance-threshold" \
  "RIPPLE_PROFILE width=4 " \
  "RIPPLE_PROFILE width=8 " \
  "RIPPLE_PROFILE width=16 " \
  "RIPPLE_PROFILE width=32 "
do
  grep -q "${witness}" "${ripple_log}"
done

shift_log="${TMP_ROOT}/shift32-profile.log"
cargo_pinned test \
  --release \
  --manifest-path "${A_CIRCUIT_MANIFEST}" \
  m4_shift32_profile_candidate_selectivity \
  -- --ignored --nocapture --test-threads=1 2>&1 | tee "${shift_log}"

for witness in \
  "SHIFT32_PROFILE op=SHL " \
  "SHIFT32_PROFILE op=SHR " \
  "SHIFT32_PROFILE op=SAR " \
  "SHIFT32_PROFILE_NOTE=informational-only-no-performance-threshold"
do
  grep -q "${witness}" "${shift_log}"
done

echo "=== OPTIMIZED CPU INFORMATIONAL BASELINES ==="
cargo_pinned test \
  --release \
  --manifest-path "${OPTIMIZED_MANIFEST}" \
  optimized_hash_index_benchmark_baseline \
  -- --ignored --nocapture --test-threads=1

echo "=== REFERENCE CPU INFORMATIONAL BASELINE ==="
cargo_pinned test \
  --release \
  --manifest-path "experiments/browser-accelerator/rust/Cargo.toml" \
  reference_cpu_benchmark_baseline \
  -- --ignored --nocapture --test-threads=1

echo "SCALE_PERFORMANCE_CLASS=PASS"
echo "SCALE_PERFORMANCE_POLICY=informational-only-no-performance-threshold"
echo "SCALE_PERFORMANCE_STORAGE_MAX_LINKS=10000000"
