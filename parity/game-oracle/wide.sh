#!/usr/bin/env bash
# Large-scale check against the real game: runs WideOracle.java on many random
# seeds into parity/.cache/wide/ (not committed), then compares Chunkscope with
# it (crates/mc-core/tests/wide.rs), printing every difference with its seed
# and position.
#
# Usage: parity/game-oracle/wide.sh [seeds] [salt] [stronghold every n-th seed]
#   defaults: 200 seeds, a salt from the date, strongholds on every 10th seed.
# The same salt gives the same seeds, so a reported difference can be rerun.
# Needs what run.sh needs (Java 25, with javac), and downloads the server jar
# the same way (fetch.sh).
set -euo pipefail

SEEDS="${1:-200}"
SALT="${2:-$(date -u +%Y%m%d)}"
STRONGHOLD_EVERY="${3:-10}"
JAVA="${JAVA:-java}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# The version run.sh pins.
VERSION=$(sed -n 's/^VERSION="${1:-\(.*\)}"$/\1/p' "$ROOT/parity/game-oracle/run.sh")
CACHE="$ROOT/parity/.cache/$VERSION"
OUT="$ROOT/parity/.cache/wide"

# Downloads the server jar (once) and unpacks it; sets CP.
source "$ROOT/parity/game-oracle/fetch.sh"

rm -rf "$OUT"
mkdir -p "$OUT/classes"
# WideOracle reuses GameOracle's helpers: compile both.
"${JAVAC:-javac}" -nowarn -cp "$CP" -d "$OUT/classes" \
  "$ROOT/parity/game-oracle/GameOracle.java" "$ROOT/parity/game-oracle/WideOracle.java"
echo "wide oracle: $SEEDS seeds, salt $SALT"
# Run from the cache dir: the game writes logs/ into the working directory.
(cd "$CACHE" && "$JAVA" -cp "$CP:$OUT/classes" WideOracle "$OUT" "$SEEDS" "$SALT" "$STRONGHOLD_EVERY")

cd "$ROOT"
CHUNKSCOPE_WIDE="$OUT" cargo test --release -p mc-core --test wide -- --nocapture --test-threads 1
