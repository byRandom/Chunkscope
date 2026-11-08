# Parity with Minecraft: Java Edition

The whole project relies on one promise: **the same seed gives the same result
as the game**. A seed finder that is right 99.9% of the time is wrong for
someone. So nothing gets merged unless it is checked against an oracle.

## Oracle tiers

| Tier | Oracle                        | Covers                                                   | Status  |
| ---- | ----------------------------- | -------------------------------------------------------- | ------- |
| 1    | The real JDK (`Oracle.java`)  | `java.util.Random`, `StrictMath`, string/number parsing, any formula written in plain Java | Done    |
| 2    | The real game (headless dump) | Xoroshiro seeding, noise, biomes, structures, spawn       | Planned |
| 3    | cubiomes (cross-check)        | Large-scale differential testing of biomes and structures | Planned |

### Tier 1: JDK oracle

`parity/oracle/Oracle.java` runs on a real JDK and writes reference outputs to
`parity/fixtures/`. The Rust tests (`crates/*/tests/parity.rs`) recompute the
same values and must match **bit for bit**: floats are compared by their raw
bits, not with a tolerance.

The oracle also generates `crates/java-compat/src/digit_table.rs`, the table of
Unicode digits accepted by `Character.digit`. We don't keep a hand-written copy.

CI regenerates every fixture with JDK 21 and fails if anything changes. This
means the committed fixtures always really come from Java.

Use the JDK major version the target Minecraft version ships with (Java 21 for
1.20.5+). Unicode tables can change between JDK versions.

### Tier 2: game oracle (planned)

Generation logic that only exists inside the game (biomes, structures) will be
checked against data dumped from a real Java Edition server. This needs a
small mod or datapack that writes biome/structure data for chosen seeds and
coordinates. Only the dumped data is committed, never game code.

## Pitfalls already caught

Each of these breaks parity in a naive port. Each is covered by the fixtures:

- **`StrictMath.log` ≠ Rust `f64::ln`.** Java uses fdlibm. With the platform
  `ln`, about 5% of inputs differ in the last bit. That changes
  `nextGaussian`, and so every later value from the RNG.
- **`String.trim()` ≠ `str::trim()`.** Java only strips chars `<= U+0020`.
  `" 42"` (no-break space + 42) is the seed `155422` (a hash), not `42`.
- **`Long.parseLong` accepts Unicode digits.** `"١٢٣"` (Arabic-Indic) and
  `"１２３"` (fullwidth) both parse to `123`. Non-BMP digits do not.
- **`String.hashCode` works on UTF-16 code units**, not Unicode scalars, so
  emoji count as two units.
- **Mixed `int`/`long` overflow** in the slime chunk formula: some products wrap
  at 32 bits, others at 64.
- **`nextInt(bound)` rejection loop** uses Java `int` overflow in its loop
  condition.
