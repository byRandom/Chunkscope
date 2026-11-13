# Glossary

The words Chunkscope's code and docs use for Minecraft world generation.
Names in the code follow these.

## Worlds

- **Seed**: the 64-bit number a world is generated from. Text seeds are
  hashed like the game does (`parse_seed`).
- **Dimension**: the Overworld, the Nether or the End. Each has its own
  biome source, terrain and structure sets.
- **Biome source**: what gives the biome at a position (by climate in the
  Overworld and the Nether, by island density in the End).
- **Terrain**: the blocks of a column before features: final density,
  aquifers, heightmaps.

## Structures

- **Structure set**: a group of structures that share one placement, such
  as `minecraft:villages` or `minecraft:nether_complexes` (fortresses and
  bastions). Generated from the game's data.
- **Placement**: where a set may place structures. Random spread splits the
  world into square regions of `spacing` chunks; concentric rings place the
  strongholds.
- **Candidate**: a chunk where a set tries to place a structure. It passed
  the placement checks (region offset, frequency, exclusion zones) but not
  yet biome and terrain. Its **locate position** is the block `/locate`
  reports.
- **Attempt**: one structure of a set tried at a candidate, in the game's
  order. The set's outcome is that the last attempt generates, that none
  does, or (for structures not ported yet) that it is unknown.
- **Generation point**: the validated start of a structure that generates.
- **Hit**: a candidate whose structure generates and passes a set filter
  (for example only bastions, or only end cities with a ship).
- **Chunk rectangle**: an area of chunks, both ends inclusive, that a scan
  covers.

## The app

- **Layer**: one row of the map's structure list: a structure set (or part
  of one), the strongholds, or the spawn.
- **Marker**: a hit drawn on the map.
- **Possible**: a candidate whose structure is not validated yet, so the
  game may skip it.
- **Estimate**: the spawn block, which needs features Chunkscope does not
  generate.

## Parity

- **Oracle**: a Java program that runs the game's (or the JDK's) own code
  and writes fixtures the Rust tests compare against, bit for bit.
- **Fixture**: the oracle's output, committed under `parity/fixtures/`.
- **Equivalent mutation**: a change to a port that no input can tell apart
  from the original, recorded in `docs/PARITY.md`.
