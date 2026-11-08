# Parity with Minecraft: Java Edition

The whole project relies on one promise: **the same seed gives the same result
as the game**. A seed finder that is right 99.9% of the time is wrong for
someone. So nothing gets merged unless it is checked against an oracle.

Target version: **Minecraft 26.3** (Java 25). The pinned version lives in
`parity/game-oracle/run.sh`; the version the fixtures came from is recorded in
`parity/fixtures/game/VERSION`.

## Oracle tiers

| Tier | Oracle                                   | Covers                                                                                     | Status      |
| ---- | ---------------------------------------- | ------------------------------------------------------------------------------------------ | ----------- |
| 1    | The real JDK (`parity/oracle`)           | `java.util.Random`, `StrictMath`, string/number parsing, any formula written in plain Java | Done        |
| 2    | The real game classes (`parity/game-oracle`) | Random sources, seeding helpers, slime chunks, Perlin and normal noise (all vanilla noises), overworld climate and biomes, structure placement; next: terrain height, structure validation, strongholds, spawn | In progress |
| 3    | cubiomes (cross-check)                   | Large-scale differential testing of biomes and structures                                  | Planned     |

In every tier, the Rust tests recompute the same values and must match **bit
for bit**. Floats are compared by their raw bits, never with a tolerance. CI
regenerates every fixture from scratch and fails if anything changes. This
means the committed fixtures always really come from the oracle.

### Tier 1: JDK oracle

`parity/oracle/Oracle.java` runs on a plain JDK and writes reference outputs
to `parity/fixtures/`. It also generates
`crates/java-compat/src/digit_table.rs`, the table of Unicode digits accepted
by `Character.digit`. We don't keep a hand-written copy.

```bash
java parity/oracle/Oracle.java
```

Use the JDK major version the target Minecraft version ships with (Java 25 for
26.x). Unicode tables can change between JDK versions.

### Tier 2: game oracle

`parity/game-oracle/run.sh` downloads the official server jar from Mojang
(verified by SHA-1) into the git-ignored `parity/.cache/`. It puts the game and
its bundled libraries on the classpath and runs `GameOracle.java`, which calls
the game's own classes (`XoroshiroRandomSource`, `WorldgenRandom`, ...) and
writes `parity/fixtures/game/`.

```bash
parity/game-oracle/run.sh          # or: parity/game-oracle/run.sh <version>
```

Only the generated data is committed, never game code. The oracle also
generates `crates/mc-core/src/noise/vanilla.rs`: the parameters of every
vanilla `worldgen/noise` file, decoded with the game's own codec. These are
numeric parameters, not code. For biomes it bootstraps the game's registries
(`VanillaRegistries.createWorldLookup()`, no world or server needed) and
generates `crates/mc-core/src/biome/generated.rs`: the overworld terrain
offset spline and the overworld biome parameter list. It also generates
`crates/mc-core/src/structure/generated.rs`, every vanilla structure set. Since 26.1 the game is
shipped without obfuscation, so the oracle can use the real class and method
names.

## Pitfalls already caught

Each of these breaks parity in a naive port. Each is covered by the fixtures:

- **`StrictMath.log` ≠ Rust `f64::ln`.** Java uses fdlibm. With the platform
  `ln`, about 5% of inputs differ in the last bit. That changes
  `java.util.Random.nextGaussian`.
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
- **Xoroshiro's `nextInt(bound)` is a different algorithm** (Lemire's
  multiply-shift) from the LCG's. A naive modulo already differs on
  `nextInt(5)` for seed 0.
- **`WorldgenRandom` draws bits differently depending on what it wraps.** Over
  an LCG it calls the LCG's `next(bits)`. Over Xoroshiro it uses
  `nextLong() >>> (64 - bits)` and then applies the `java.util.Random`
  algorithms on top. `fork()` and `setSeed()` go to the wrapped source.
- **Positional factories differ.** Xoroshiro's `fromHashOf` uses MD5. Legacy's
  uses `String.hashCode`. Legacy's `fromSeed` ignores the factory seed
  entirely.

- **Noise is `float` since 26.1.** `PerlinNoise.get` finds the lattice cell in
  `double`, then does the gradients, smoothstep, interpolation and the octave
  sum in `float`. A port of the older all-`double` noise (cubiomes, Amidst and
  others) gives different values for 26.x worlds.
- **Coordinates are wrapped to a 2^25 window** (`GradientNoise.wrap`) before
  sampling, so far-out coordinates keep their precision. The window edge is
  `Math.nextDown(2^24)`.
- **Normal noise is normalized by the octaves' total amplitude**, not by
  `base_amplitude`. That total is computed with `DoubleStream.sum()`, which
  uses Kahan compensated summation. A plain sum changes the normalization
  factor in the last bit.
- **Each octave uses two Perlin noises**, seeded by hashing `"octave_<index>"`
  with two separate positional factories. The second one samples at
  `frequency * 1.0181268882175227`. Layers are summed in the order A0, B0, A1,
  B1, ...

- **Density functions are compiled and `float` since 26.1.** The game
  compiles `noise_router` functions into samplers, and the compiler rewrites
  the tree. Climate noises get sliced at y = 0, `shift_a` becomes
  `noise * 4`, and the blending `lerp` returns its second argument unchanged
  when `alpha == 1`. Chunkscope evaluates the overworld climate functions in
  the same compiled form.
- **The terrain spline is `float` too.** The cubic Hermite interpolation must
  keep the game's exact operation order:
  `lerp(t, v0, v1) + t * (1 - t) * lerp(t, p, q)`. Regrouping the product
  changes `depth` by one ulp.
- **Climate values are quantized with `(long) (v * 10000f)`**, a float
  multiply followed by truncation.

- **The `default` frequency reducer passes its arguments in the wrong
  order.** `probabilityReducer` calls
  `setLargeFeatureWithSalt(seed, salt, chunkX, chunkZ)`, where the method
  expects `(seed, regionX, regionZ, salt)`. Chunkscope keeps the game's order.
  No vanilla structure set uses `default` with a frequency below 1, so the
  placement fixtures cannot catch this. A separate fixture calls all four
  reducers directly.

## Known non-determinism in the game itself

Minecraft's own `RandomSource.nextGaussian()` (`MarsagliaPolarGaussian`) uses
`Math.log`, not `StrictMath.log`. `Math.log` is a HotSpot intrinsic. On x86_64
it differs from `StrictMath.log` in about 7% of inputs (measured on OpenJDK 21
and 25, interpreted and JIT-compiled alike), and other CPU architectures use
different implementations. Gaussian values from the game's random sources can
therefore depend on the player's CPU. Chunkscope does not implement them until
a world-generation feature needs them; when one does, it must match the
x86_64 HotSpot result and document the caveat.

`java.util.Random.nextGaussian()` uses `StrictMath` and is deterministic; it is
implemented in `java-compat`.

### Biome lookup depends on query order

The R-tree search in `Climate.RTree` always finds the minimum distance, but
when several biome boxes are at that same distance (for example, the point
lies inside more than one), the winner depends on where the search started.
It starts from the previous query's result, which the game keeps in a
`ThreadLocal`, and only replaces it on a strictly smaller distance. So the
same climate point can map to different biomes depending on what was looked
up before on the same thread. For example, seed 0 at quart (96, 16, -192)
gives `sparse_jungle` after the oracle's query sequence and `plains` on a
fresh tree. In the game, this means some biome borders depend on chunk
generation order.

Chunkscope reproduces the game's search exactly, including that state.
`ParameterList` keeps the last result, and the tests replay the oracle's
query order on a fresh tree. A map rendered in a different order can
therefore differ from the game in rare border cells. The game itself is not
deterministic there either.
