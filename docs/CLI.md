# Command line

`chunkscope-cli` runs the same world generation as the app, verified against
the game, from a terminal or a script. It ships in the release archives next
to the app; from source:

```bash
cargo run --release -p chunkscope-cli -- <command> [options]
```

A **seed** is a number or any text, like the game's world creation screen
(`hello` is seed 99162322). A blank seed is an error. Coordinates are block
positions.

Exit codes: `0` success, `2` a usage mistake (the reason goes to stderr,
nothing to stdout).

## Commands

| Command | What it prints |
| --- | --- |
| `biome <seed> <x> <z> [y]` | the biome at a block (default y 64) |
| `height <seed> <x> <z>` | `WORLD_SURFACE_WG` and `OCEAN_FLOOR_WG` of a column, and what its blocks are made of |
| `spawn <seed>` | the world spawn origin (exact) and the estimated spawn block (Overworld) |
| `structures <seed> <x> <z> [radius]` | the structures within `radius` blocks (default 1000), nearest first, with strongholds in the Overworld and the 20 gateways in the End (`end_gateway #n`: the nth dragon defeat opens it; JSON `order`) and the return gateways of the outer islands (`end_gateway_return`; JSON `y` and `under_city`) |
| `find [options] <query>...` | the seeds that match a query |
| `targets` | the structure targets a query can name |
| `help`, `version` | |

Options:

- `--dimension overworld|nether|end` (`biome`, `height`, `structures`):
  default `overworld`.
- `--json` (every command): JSON instead of text.
- `--help` after a command: its options.

`find` options: `--from <seed>` (first seed, default 0), `--random` (random
seeds; `--from` is the salt), `--count <n>` (default 10000), `--threads <n>`
(default all cores). The rest is the query, the same text the app's finder
copies and pastes:

```text
center=spawn|zero  <target>[+ship]:<radius>[:<count>]  biome=<biome>,...
```

```bash
chunkscope-cli find --count 100000 villages:300 pillager_outposts:500 biome=plains
```

Matches print as they are found, one per line (the app's "copy all
results" line), on stdout; the query and the progress summary go to stderr.

## JSON

One JSON object per command (`find`: one per match, one per line). Fields are
only ever added, never renamed or removed, within a major version; read the
ones you need and ignore the rest. Coordinates are integers, distances have
one decimal, names are the game's (`minecraft:plains`).

```text
biome       {"seed","dimension","x","y","z","biome"}
height      {"seed","dimension","x","z","world_surface","ocean_floor",
             "column":{"solid","air","water","lava"}}
spawn       {"seed","origin":{"x","z"},"chunk":{"x","z"},"estimate":{"x","y","z"}}
structures  {"seed","dimension","center":{"x","z"},"radius",
             "structures":[{"structure","x","z","distance"[,"order"][,"y","under_city"]}]}
find        {"seed","center":{"x","z"},"biome",
             "found":[{"structure","dimension","x","z","distance"}]}
targets     [{"id","dimension"}]
```
