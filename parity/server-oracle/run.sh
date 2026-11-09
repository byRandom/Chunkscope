#!/usr/bin/env bash
# Server oracle: starts the official Minecraft server once per seed and
# records the world spawn it picks (parity/fixtures/server/spawn.txt).
#
# The spawn block depends on fully generated chunks (trees, lakes, ice), so
# only a real server gives it; Chunkscope's spawn is an estimate, and this
# fixture measures how close it is. It takes about 15 s per seed, so it does
# not run in CI: regenerate it by hand when the target version changes.
#
# Running the server means accepting the Minecraft EULA
# (https://aka.ms/MinecraftEULA). Set MINECRAFT_EULA=true to confirm you do.
#
# Usage: MINECRAFT_EULA=true parity/server-oracle/run.sh [version]
set -euo pipefail

if [[ "${MINECRAFT_EULA:-}" != "true" ]]; then
  echo "Running the Minecraft server requires accepting the Minecraft EULA" >&2
  echo "(https://aka.ms/MinecraftEULA). Set MINECRAFT_EULA=true to accept it." >&2
  exit 1
fi

VERSION="${1:-26.3}"
JAVA="${JAVA:-java}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
JAR="$ROOT/parity/.cache/$VERSION/server.jar"
OUT="$ROOT/parity/fixtures/server/spawn.txt"
WORK="$ROOT/parity/.cache/$VERSION/server-oracle"

if [[ ! -f "$JAR" ]]; then
  echo "No server jar: run parity/game-oracle/run.sh $VERSION first (it downloads and verifies it)." >&2
  exit 1
fi

mkdir -p "$(dirname "$OUT")"
: > "$OUT.part"
while read -r seed; do
  rm -rf "$WORK"
  mkdir -p "$WORK"
  echo "eula=true" > "$WORK/eula.txt"
  printf 'level-seed=%s\nonline-mode=false\nserver-port=25599\nview-distance=4\nsimulation-distance=4\n' "$seed" \
    > "$WORK/server.properties"
  # Start the server, wait until the world is ready, then stop it.
  (cd "$WORK" && timeout 600 bash -c '
    ( while ! grep -q "Done (" logs/latest.log 2>/dev/null; do sleep 1; done; echo stop ) |
      "$0" -Xmx2G -jar "$1" --nogui' "$JAVA" "$JAR" > server.log 2>&1)
  echo "$seed $(python3 -I "$ROOT/parity/server-oracle/level_spawn.py" "$WORK/world/level.dat")" >> "$OUT.part"
done < "$ROOT/parity/server-oracle/seeds.txt"
echo "$VERSION" > "$(dirname "$OUT")/VERSION"
mv "$OUT.part" "$OUT"
echo "server spawns written for Minecraft $VERSION"
