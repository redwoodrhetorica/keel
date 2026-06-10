#!/usr/bin/env bash
# All-sectors fuzz soak (the completion-gate procedure, Addendum 177).
# Usage: bash soak_sectors.sh [seconds-per-target] [target ...]
set -u
. ~/.cargo/env
cd "$(dirname "$0")/.."
secs="${1:-2400}"
shift || true
targets=("$@")
if [ "${#targets[@]}" -eq 0 ]; then
  targets=(fuzz_boolean fuzz_cyl_boolean fuzz_nurbs_boolean fuzz_imprint \
    fuzz_topo_ops fuzz_winding fuzz_pmc fuzz_ssi fuzz_step_import \
    fuzz_recover fuzz_nurbs_curve fuzz_nurbs_surface fuzz_bernstein_roots \
    fuzz_interval fuzz_solve_cubic)
fi
for t in "${targets[@]}"; do
  echo "=== $t ==="
  CARGO_TARGET_DIR=~/keel-fuzz-target cargo +nightly fuzz run "$t" -- \
    -max_total_time="$secs" 2>&1 | tail -1
done
echo ALL-SECTOR-SOAK-DONE
