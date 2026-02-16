#!/usr/bin/env bash
# Generate callgrind fixture .out files by compiling small Rust programs
# and running them through valgrind --tool=callgrind inside Docker (nixery).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FIXTURES="$REPO_ROOT/gungraun-runner/tests/fixtures/callgrind.out"
SRC_DIR="$SCRIPT_DIR/fixture_src"

ARCH_PREFIX=""
if [ "$(uname -m)" = "arm64" ]; then
    ARCH_PREFIX="arm64/"
fi
IMAGE="nixery.dev/${ARCH_PREFIX}shell/gcc/rustc/valgrind"

docker run --rm \
    -v "$SRC_DIR:/src:ro" \
    -v "$FIXTURES:/out" \
    --security-opt seccomp=unconfined \
    "$IMAGE" \
    sh -c "
        set -e
        mkdir -p /work && cd /work

        # Compile all binaries
        for src in /src/benchmark-tests-*.rs; do
            name=\$(basename \"\$src\" .rs)
            cp \"\$src\" \"\$name.rs\"
            rustc -C opt-level=0 -C debuginfo=2 -o \"\$name\" \"\$name.rs\"
        done

        # Common callgrind flags matching gungraun's defaults
        CG='--compress-strings=no --compress-pos=no --dump-line=yes'

        # Run callgrind on each
        # benchmark-tests-exit: full run (no_entry_point) and toggle-collect on main (when_entry_point)
        valgrind --tool=callgrind \$CG --callgrind-out-file=/out/callgrind.no_entry_point.out ./benchmark-tests-exit 0
        valgrind --tool=callgrind \$CG --collect-atstart=no --toggle-collect=benchmark_tests_exit::main --callgrind-out-file=/out/callgrind.when_entry_point.out ./benchmark-tests-exit 0
        valgrind --tool=callgrind \$CG --collect-atstart=no --toggle-collect=benchmark_tests_branching::main --callgrind-out-file=/out/callgrind.branching.out ./benchmark-tests-branching 0
        valgrind --tool=callgrind \$CG --collect-atstart=no --toggle-collect=benchmark_tests_recursive::main --callgrind-out-file=/out/callgrind.recursive.out ./benchmark-tests-recursive 0
    "

echo "Generated:"
for f in "$FIXTURES"/callgrind.{no_entry_point,when_entry_point,branching,recursive}.out; do
    echo "  $f"
done
