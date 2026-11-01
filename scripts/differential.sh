#!/usr/bin/env bash
# Differential tests: Rust format readers against the C++ ones on real data.
#
#   nix-shell --run scripts/differential.sh [client dir] [cargo test filter]
#
# Builds tools/differential (C++ reference dumpers) into build/differential,
# then runs the ignored differential_* tests of m2-formats.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
client="$(cd "${1:-$repo/bin}" && pwd)"
filter="${2:-}"
build="$repo/build/differential"

cmake -S "$repo" -B "$build" -G Ninja \
    -DCMAKE_BUILD_TYPE=RelWithDebInfo \
    -DM2_UNITY_BUILD=ON \
    -DM2_BUILD_DIFF_TOOLS=ON \
    -DBUILD_TESTS=OFF >/dev/null

mapfile -t targets < <(cmake --build "$build" --target help | sed -n 's/^\(m2diff_[a-z0-9_]*\):.*/\1/p')
cmake --build "$build" --target "${targets[@]}"

export M2_CLIENT_DIR="$client"
export M2_DIFF_BUILD="$build"
tests=()
for f in "$repo"/crates/m2-formats/tests/differential_*.rs; do
    tests+=(--test "$(basename "$f" .rs)")
done
cargo test --release -p m2-formats "${tests[@]}" -- --ignored --nocapture ${filter:+"$filter"}
