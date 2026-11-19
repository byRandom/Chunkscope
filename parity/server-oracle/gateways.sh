#!/usr/bin/env bash
# Server oracle for the End's return gateways: for a few seeds, loads the
# chunks where Chunkscope predicts return gateways on the official server and
# records whether an end gateway block entity is really at the predicted block
# (parity/fixtures/server/end_return_gateways.txt: `seed x y z yes|no`). Each
# prediction is also asked one block higher, which must be `no`, so the check
# cannot pass by accident. The last field of a line says whether an end city's
# piece covers the gateway's column (`covered`: its blocks can raise the
# surface, so the gateway can be higher than predicted), an end city starts
# within 48 blocks (`city`) or neither (`plain`).
#
# About 20 s per seed plus a few seconds per gateway; not run in CI.
# Running the server means accepting the Minecraft EULA
# (https://aka.ms/MinecraftEULA). Set MINECRAFT_EULA=true to confirm you do.
#
# Usage: MINECRAFT_EULA=true parity/server-oracle/gateways.sh [version]
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
OUT="$ROOT/parity/fixtures/server/end_return_gateways.txt"
WORK="$ROOT/parity/.cache/$VERSION/server-oracle-gateways"
PER_SEED="${PER_SEED:-3}"

if [[ ! -f "$JAR" ]]; then
  echo "No server jar: run parity/game-oracle/run.sh $VERSION first." >&2
  exit 1
fi
cargo build --release -q -p chunkscope-cli --manifest-path "$ROOT/Cargo.toml"
CLI="$ROOT/target/release/chunkscope-cli"

mkdir -p "$(dirname "$OUT")"
: > "$OUT.part"
while read -r seed; do
  # The nearest gateways, and any that is near an end city.
  mapfile -t gateways < <("$CLI" structures "$seed" 0 0 12000 --dimension end --json | python3 -I -c '
import json, sys
d = json.load(sys.stdin)["structures"]
gates = [s for s in d if s["structure"] == "minecraft:end_gateway_return"]
cities = [s for s in d if s["structure"] == "minecraft:end_city"]
def near_city(g):
    return any(abs(g["x"] - c["x"]) <= 48 and abs(g["z"] - c["z"]) <= 48 for c in cities)
head = gates[:int(sys.argv[1])]
rest = [g for g in gates[int(sys.argv[1]):]]
picked = head + [g for g in rest if g["under_city"]][:3] + [g for g in rest if near_city(g) and not g["under_city"]][:1]
for g in picked:
    print(g["x"], g["y"], g["z"], "covered" if g["under_city"] else ("city" if near_city(g) else "plain"))
' "$PER_SEED")
  [[ ${#gateways[@]} -gt 0 ]] || continue
  args=()
  for g in "${gateways[@]}"; do
    read -r x y z _ <<< "$g"
    args+=("$x,$y,$z" "$x,$((y + 1)),$z")
  done
  rm -rf "$WORK"
  python3 -I "$ROOT/parity/server-oracle/gateways.py" "$JAVA" "$JAR" "$WORK" "$seed" "${args[@]}" > "$WORK.out"
  # Attach the city tag to the exact-height lines.
  python3 -I - "$WORK.out" "${gateways[@]}" >> "$OUT.part" <<'PY'
import sys
tags = {}
for g in sys.argv[2:]:
    x, y, z, tag = g.split()
    tags[(int(x), int(y), int(z))] = tag
for line in open(sys.argv[1]):
    seed, x, y, z, answer = line.split()
    key = (int(x), int(y), int(z))
    print(seed, x, y, z, answer, tags.get(key, "above"))
PY
done < "$ROOT/parity/server-oracle/gateway-seeds.txt"
echo "$VERSION" > "$(dirname "$OUT")/VERSION"
mv "$OUT.part" "$OUT"
echo "server return gateways written for Minecraft $VERSION"
