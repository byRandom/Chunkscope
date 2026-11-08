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
MANIFEST=https://piston-meta.mojang.com/mc/game/version_manifest_v2.json

mkdir -p "$CACHE"
if [[ ! -f "$CACHE/server.jar" ]]; then
  version_url=$(curl -fsSL "$MANIFEST" | python3 -c "
import json, sys
d = json.load(sys.stdin)
print(next(v['url'] for v in d['versions'] if v['id'] == sys.argv[1]))" "$VERSION")
  read -r url sha1 < <(curl -fsSL "$version_url" | python3 -c "
import json, sys
s = json.load(sys.stdin)['downloads']['server']
print(s['url'], s['sha1'])")
  curl -fsSL -o "$CACHE/server.jar.part" "$url"
  echo "$sha1  $CACHE/server.jar.part" | sha1sum -c --quiet
  mv "$CACHE/server.jar.part" "$CACHE/server.jar"
fi

# The server jar is a bundler: the game and its libraries are nested jars.
rm -rf "$CACHE/bundle"
unzip -qo "$CACHE/server.jar" 'META-INF/versions/*' 'META-INF/libraries/*' -d "$CACHE/bundle"
CP=$(find "$CACHE/bundle/META-INF" -name '*.jar' | sort | paste -sd: -)

# Run from the cache dir: the game writes logs/ into the working directory.
cd "$CACHE"
"$JAVA" -cp "$CP" "$ROOT/parity/game-oracle/GameOracle.java" "$OUT" "$VERSION"
