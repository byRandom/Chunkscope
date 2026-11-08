# seedmap

> Working name — subject to change.

A fast, local, open-source seed map and seed finder for **Minecraft: Java
Edition**, written in Rust. Think of an offline tool that shows biomes,
structures and slime chunks for any seed, and can search millions of seeds for
the features you care about.

**Status:** early. The foundation (bit-exact Java RNG and seed handling) is
done and verified against a real JDK; world generation is next.

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
| `crates/java-compat`   | Bit-exact ports of the JDK behaviour Minecraft relies on: `java.util.Random`, `StrictMath.log`, `String.hashCode`, `String.trim`, `Long.parseLong` |
| `crates/mc-core`       | Minecraft logic: seed text → world seed, slime chunks                                   |
| `parity/`              | Java oracle that produces reference fixtures from a real JDK                            |

## Roadmap

- [x] `java.util.Random` (including `nextGaussian` via an fdlibm port)
- [x] Seed text parsing exactly as the game does it
- [x] Slime chunks
- [ ] Xoroshiro128++ and Minecraft's `RandomSupport` seeding (1.18+)
- [ ] Perlin / octave / double-Perlin noise
- [ ] Biome generation (starting with 1.21)
- [ ] Structure placement and biome validation
- [ ] Strongholds and world spawn
- [ ] Seed search CLI with filters
- [ ] Map viewer GUI

## Development

```bash
cargo test
```

To regenerate the reference fixtures (requires JDK 21):

```bash
java parity/oracle/Oracle.java
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
