# Chunkscope

A fast, local, open-source seed map and seed finder for **Minecraft: Java
Edition**, written in Rust. Think of an offline tool that shows biomes,
structures and slime chunks for any seed, and can search millions of seeds for
the features you care about.

**Status:** early. Overworld biomes already match the game: the random
number generators, seed handling, noise, climate sampling and biome lookup
are all verified against the real JDK and the real game classes. Structure
placement (where each structure set attempts to generate) and stronghold
positions match too; the biome and terrain checks that decide whether a structure really generates
there are next.

Target: Minecraft **26.3** (Java 25).

## Goals

- **Bit-exact parity with Java Edition.** If the tool says there is a village
  at a position, there is one in the game. Every piece of logic is tested
  against an oracle (see [docs/PARITY.md](docs/PARITY.md)).
- **Local and fast.** No server, no uploads. Multi-threaded seed search.
- **Pure Rust.** No C dependencies.

Bedrock Edition is out of scope for now.

## Workspace

| Crate                  | Purpose                                                                                 |
| ---------------------- | --------------------------------------------------------------------------------------- |
| `crates/java-compat`   | Bit-exact ports of the JDK behaviour Minecraft relies on: `java.util.Random`, `StrictMath.log`, `Math.round`, `DoubleStream.sum`, `String.hashCode`, `String.trim`, `Long.parseLong` |
| `crates/mc-core`       | Minecraft logic: random sources (Xoroshiro, legacy, worldgen), noise (Perlin, normal noise, all vanilla noise parameters), overworld climate and biomes, overworld terrain (`final_density`, aquifers, heightmaps), structure placement, seed text → world seed, slime chunks |
| `parity/oracle`        | JDK oracle: reference fixtures from a real JDK                                          |
| `parity/game-oracle`   | Game oracle: reference fixtures from the real game classes (downloads the official server jar) |

## Roadmap

- [x] `java.util.Random` (including `nextGaussian` via an fdlibm port)
- [x] Seed text parsing exactly as the game does it
- [x] Slime chunks
- [x] Xoroshiro128++, `RandomSupport` seeding and positional random factories
- [x] `WorldgenRandom` decoration / feature / structure seeding
- [x] Perlin noise and normal (double-Perlin) noise, with all vanilla noise parameters
- [ ] Smeared Perlin, blended and simplex noise (terrain, End islands)
- [x] Overworld biomes (climate sampling, terrain offset spline, biome R-tree)
- [ ] Large biomes / amplified presets, Nether and End biomes
- [x] Structure placement: candidate chunks, frequency rolls, exclusion zones (all vanilla structure sets)
- [x] Strongholds (concentric rings with their biome search)
- [ ] Terrain height (needed to validate structures), in steps:
  - [x] Volume-mode noise (`NoiseStack.addToVolume`)
  - [x] Volume-mode density samplers, cell interpolation, smeared Perlin noise
  - [x] Overworld `final_density` (the game's compiled sampler tree)
  - [x] Aquifers
  - [x] Heightmaps (`getBaseHeight`: `WORLD_SURFACE_WG`, `OCEAN_FLOOR_WG`) and base block columns
- [ ] Structure validation (biome and terrain height)
- [ ] Strongholds and world spawn
- [ ] Seed search CLI with filters
- [ ] Map viewer GUI

## Try it

```bash
cargo run --release -p mc-core --example biome -- <seed> <x> <z> [y]
```

```bash
cargo run --release -p mc-core --example structures -- <seed> <x> <z> [radius]
```

```bash
cargo run --release -p mc-core --example height -- <seed> <x> <z>
```

## Development

```bash
cargo test
```

To regenerate the reference fixtures (requires JDK 25):

```bash
java parity/oracle/Oracle.java
```

```bash
parity/game-oracle/run.sh
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this work, as defined in the Apache-2.0 license, shall be dual
licensed as above, without any additional terms or conditions.

This project is not affiliated with Mojang Studios or Microsoft. "Minecraft" is
a trademark of Mojang Synergies AB. No game code or assets are included.
