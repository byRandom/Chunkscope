#!/usr/bin/env bash
# Regenerates parity/fixtures/game/ by running the real Minecraft server
# classes. Downloads the official server jar from Mojang (verified by SHA-1)
# into parity/.cache/, which is git-ignored. Game code is never committed.
#
# Usage: parity/game-oracle/run.sh [version]   (default: the pinned version below)
# Needs a JDK matching the version's javaVersion (Java 25 for 26.x) as `java`,
# or set JAVA to its path.
set -euo pipefail

VERSION="${1:-26.3}"
JAVA="${JAVA:-java}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CACHE="$ROOT/parity/.cache/$VERSION"
OUT="$ROOT/parity/fixtures/game"

# Downloads the server jar (once) and unpacks it; sets CP.
source "$ROOT/parity/game-oracle/fetch.sh"

# Run from the cache dir: the game writes logs/ into the working directory.
cd "$CACHE"
"$JAVA" -cp "$CP" "$ROOT/parity/game-oracle/GameOracle.java" "$OUT" "$VERSION" \
  "$ROOT/crates/mc-core/src/noise/vanilla.rs" "$ROOT/crates/mc-core/src/biome/generated.rs" \
  "$ROOT/crates/mc-core/src/structure/generated.rs" "$ROOT/crates/mc-core/src/density/generated.rs"
