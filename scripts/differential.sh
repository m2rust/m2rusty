#!/usr/bin/env bash
# Differential tests: Rust against the C++ reference on real client data.
#
#   nix-shell --run scripts/differential.sh [client dir] [cargo test filter]
#
# Builds tools/differential (C++ reference dumpers) into build/differential,
# then runs every crates/*/tests/differential*.rs suite. That covers the format
# readers (m2-formats) as well as the world, effect, UI and networking ports.
# A test file whose tests are `#[ignore]`d is run with --ignored; the networking
# suite is a plain test that skips itself when its dumper is missing.
set -uo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
client="$(cd "${1:-$repo/bin}" && pwd)"
filter="${2:-}"
build="$repo/build/differential"

cmake -S "$repo" -B "$build" -G Ninja \
    -DCMAKE_BUILD_TYPE=RelWithDebInfo \
    -DM2_UNITY_BUILD=ON \
    -DM2_BUILD_DIFF_TOOLS=ON \
    -DBUILD_TESTS=OFF >/dev/null || exit 1

mapfile -t targets < <(cmake --build "$build" --target help | sed -n 's/^\(m2diff_[a-z0-9_]*\):.*/\1/p')
cmake --build "$build" --target "${targets[@]}" || exit 1

export M2_CLIENT_DIR="$client"
export M2_DIFF_BUILD="$build"

failed=()
passed=()
for f in "$repo"/crates/*/tests/differential*.rs; do
    [ -e "$f" ] || continue
    crate="$(basename "$(dirname "$(dirname "$f")")")"
    test="$(basename "$f" .rs)"
    ignored=()
    if grep -q '#\[ignore' "$f"; then
        ignored=(--ignored)
    fi
    echo "== $crate $test"
    start=$SECONDS
    log="$(mktemp)"
    if cargo test --release -p "$crate" --test "$test" -- "${ignored[@]}" --nocapture ${filter:+"$filter"} 2>&1 | tee "$log"; then
        # A filter can match nothing; report the count so that is visible.
        ran="$(grep -o 'test result: ok\. [0-9]* passed' "$log" | awk '{n += $4} END {print n + 0}')"
        passed+=("$crate/$test ${ran} test(s) $((SECONDS - start))s")
    else
        failed+=("$crate/$test")
    fi
    rm -f "$log"
done

echo
echo "== summary"
printf 'pass: %s\n' "${passed[@]:-none}"
if [ ${#failed[@]} -ne 0 ]; then
    printf 'FAIL: %s\n' "${failed[@]}"
    exit 1
fi
