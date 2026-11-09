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
| 2    | The real game classes (`parity/game-oracle`) | Random sources, seeding helpers, slime chunks, Perlin and normal noise (all vanilla noises), overworld climate and biomes, structure placement, strongholds, overworld terrain (`final_density` per sampler node, aquifers, heightmaps), structure validation (every overworld structure but mineshafts and ruined portals), world spawn origin, Nether biomes, terrain, fortresses and bastions | In progress |
| 2b   | Real servers (`parity/server-oracle`)    | The world spawn block (it needs fully generated chunks); run by hand, not in CI | 32 seeds |
| 3    | The game at scale (wide oracle runs)     | Thousands of seeds in every dimension, run before releases; cubiomes cannot be used, its noise is `double` and 26.x noise is `float` | Planned (0.7) |

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

- **`Math.round` is not `f64::round`.** Java rounds halves up, so `-2.5`
  becomes `-2`; Rust rounds them away from zero, so it gives `-3`. Strongholds
  use the JDK's bit-level `Math.round`, ported in `java-compat` and checked by
  the JDK oracle.
- **Stronghold biome search keeps a random match, not the first.**
  `findBiomeHorizontal` scans the whole 57×57 quart square and replaces its
  candidate with probability `1/(n+1)` using the forked ring random. Taking
  the first match moves most strongholds.

- **Terrain uses the compiled density tree, in volume mode.** The oracle
  exports the overworld `final_density` exactly as the game compiles it for
  `getBaseHeight`, so the compiler's rewrites (slices, caches, constant
  folding, `old_blended_noise` expanded into samplers) are reproduced rather
  than re-derived from JSON. A fixture records every node's output over whole
  block columns, so a mismatch names the sampler type at fault.
- **Cell interpolation accumulates along Y.** `InterpolatedFunction` fills a
  cell with `value += step` per block instead of interpolating each block;
  interpolating each block changes 140 of the 2,210 node outputs in the
  fixtures.
- **Volume and point mode differ in places.** In volume mode `Mul` has no
  "left is 0" short-circuit, `Lerp` and `RangeChoice` evaluate both branches
  over the whole volume, and interpolation only exists in volume mode. Noise
  itself gives the same bits in both modes.
- **`Mth.clamp(float)` uses Java's `Math.min`**, which handles NaN and `-0.0`
  differently from `f32::min`; `java-compat` ports it.
- **Smeared Perlin noise** (`old_blended_noise`) snaps the gradient Y to
  multiples of a per-octave smear (`computeFudgeY`) while the smoothstep uses
  the real Y. Dropping the smear changes 236 node outputs.

- **Density caches are not transparent.** A `CachingDensitySampler` keeps,
  per sampling context, the last volume it computed and the last point. A
  point query inside that cached volume returns the volume-mode value, so
  results depend on the order of queries through the same context. Volume
  and point mode can differ in the sign of zero (`Mul` short-circuits to
  `+0.0` only in point mode), so Chunkscope models the cache cells and
  replays the game's order (a fresh `NoiseChunk` per column: the aquifer
  samples the surface level first, then the final density, then blocks from
  the top down).
- **`find_top_surface` always samples density in point mode**, even when it
  is evaluated in volume mode, and the aquifer's surface-level cache is
  filled in volume mode for the chunk's area and in point mode outside it.
- **`OCEAN_FLOOR_WG` uses a block tag** (`blocks_motion_in_heightmap`), not a
  block property, in 26.x. Stone is in it; water and lava are not. The tag is
  not bound in a world-less registry lookup, so the oracle applies the tag,
  resolved from the game's data, to the game's own `getBaseColumn`.
- **Aquifers.** The 4 nearest aquifer centers are kept with the newer one
  first on equal distance; barrier stone appears where `density + pressure >
  0`. A dedicated fixture calls `computeSubstance` with densities near 0
  (where barriers decide) at every Y: breaking the tie order or a pressure
  constant fails it. Not covered yet: the lava threshold window (fluid levels
  between -12 and -10), which the fixtures never reach.

- **The Nether uses the legacy random source, and two of its noises are
  special.** With `legacy_random_source`, the game seeds the Nether's
  temperature and vegetation noises with `new LegacyRandomSource(seed)` and
  `seed + 1`, and builds them with `createForLegacyNetherBiome`: two
  `LegacyFbmInitializer` stacks drawn one after the other (the octave at
  index `-firstOctave` is always drawn first; every octave of zero
  amplitude skips 262 draws), all layers of the first stack and then all
  layers of the second, not interleaved as in `NormalNoise`. Their y scale
  is 0, so Nether biomes do not depend on the height. A fixture checks the
  climate and the biome of 4,800 lookups on 12 seeds.
- **A structure attempt is not a structure.** Where a set attempts a
  structure, the game asks the structure for a generation point and keeps
  it only if the biome *at that point* is one of the structure's biomes.
  On the fixtures' candidate chunks, only about a quarter of village
  attempts and 2% of mansion attempts generate. The point depends on the
  structure type:
  - most types first check that a valid biome exists anywhere in the
    column at the chunk center, from y = -65 to 319 (sampled in volume
    mode, which gives the same climate values as point mode);
  - `onTopOfChunkCenter` uses `getFirstOccupiedHeight`, which is the
    heightmap height **minus one**; jigsaw starts use
    `getFirstFreeHeight`, the height itself;
  - desert pyramids and jungle temples need the lowest corner of their
    footprint at sea level or above, woodland mansions at y = 60 or above,
    in a direction that depends on their random rotation;
  - ocean monuments need deep ocean in a 29-block radius; the game looks up
    every biome in that box before checking any, so all lookups happen;
  - jigsaw structures (villages, outposts, ancient cities, trail ruins,
    trial chambers, abandoned camps) start at the center of their start
    piece, `(max + min) / 2` with Java's truncating division. The piece is
    picked from the start pool with the chunk's random, after the start
    height and the rotation. The oracle exports each start element's
    bounding box per rotation and its named start jigsaw (ancient cities).
- **The Nether terrain** is the Nether's compiled `final_density`, exported
  like the overworld's, with the legacy random source: its blended noise is
  seeded with `new LegacyRandomSource(seed)`, not from the positional
  factory. It has no aquifers: where the density is not positive, blocks
  below y = 32 are lava and the rest air. A fixture checks 126 base columns
  and their `WORLD_SURFACE_WG` height.
- **Nether fortresses and bastions share a set** (`nether_complexes`,
  weights 2 and 3). A fortress always starts at its chunk's corner at
  y = 64 and only needs a Nether biome there (every Nether biome allows
  one), so the set almost never comes up empty; a bastion is a jigsaw
  structure starting at y = 33, not projected to the terrain. Neither
  needs the Nether terrain.
- **Sets with several structures retry.** Villages, ocean ruins,
  shipwrecks and abandoned camps pick a structure by weight with a random
  seeded by `setLargeFeatureSeed`; if it does not generate, the game removes
  it and picks again among the rest.
- Not ported yet: mineshafts (their height comes from their pieces) and
  ruined portals. The app shows them as possible.
- Two checks cannot be told apart by any vanilla data, so no fixture
  covers them: the bottom of the column pre-check (y = -65 instead of -64),
  and the trial chambers' dimension padding (they start at y = -40 or
  higher, far from the limit).

- **The world spawn starts from the climate.** `NoiseSpawnFinder` scores
  points by the squared distance of their quantized climate to the
  `spawn_target` ranges (times 2048²) plus their squared distance to (0, 0):
  first on rings 512 blocks apart up to 2048, then on rings 32 blocks apart
  up to 512 around the best point. The climate is sampled at the point's
  quart corner, but the distance uses the point itself, and the stored
  position is the point itself. The rings use `Math.sin` and `Math.cos`
  (see below). A fixture checks the origin of 409 seeds. No fixture can
  catch a change of the ring's angle limit (no angle step lands near 2π)
  or of the tie rule (no tie in 409 seeds); the port keeps the game's
  strict comparison.
- **The spawn block needs full chunks.** From the origin's chunk the game
  walks 11×11 chunks in a spiral and stands the player on the first column
  whose top is not a fluid, measured on fully generated chunks (trees,
  lakes, ice). Chunkscope runs the same walk on its terrain, without
  features, so its spawn block is an estimate; the app says so. Measured
  against 32 real 26.3 servers (`parity/server-oracle/run.sh`, which starts
  the official server for each seed and reads the spawn from `level.dat`):
  31 have the same x and z, 23 are exact. When only y differs, the real
  spawn is 3 to 10 blocks higher, on a tree. The one x, z miss is on a
  shore. That script runs the real server, so it needs the Minecraft EULA
  accepted (`MINECRAFT_EULA=true`) and about 15 seconds per seed; it is
  run by hand when the target version changes, not in CI.

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

### The spawn block's height

Two runs of the same 26.3 server with the same seed can stand the player at
different heights: on 2 of the 32 seeds of `parity/server-oracle`, the spawn
y changed between runs (109 and 106, 79 and 83) while x and z stayed the
same. The server generates chunks on several threads, and whether a tree
near the spawn column has been placed when the spawn is chosen depends on
that timing. So the spawn y cannot be predicted exactly, by Chunkscope or
anyone; x and z were stable in both runs.

### `Math.sin` / `Math.cos` in stronghold rings

Stronghold ring positions use `Math.cos` and `Math.sin`, which are HotSpot
intrinsics like `Math.log`. On x86_64 HotSpot they differ from `StrictMath` in
about 3.4% of inputs, and Rust's `f64::sin`/`cos` differ from HotSpot's in
about 0.2%, always in the last bit. The result is multiplied by a distance of
at most a few thousand chunks and rounded to a whole chunk, so a last-bit
difference only matters within about 1e-13 of a half. Chunkscope uses Rust's
functions. All 512 stronghold positions in the fixtures match.

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

Stronghold positions are computed by the game on background threads, each
with its own search state. Chunkscope uses a fresh search per position. In
the fixtures (4 seeds × 128 strongholds), sharing one search state across all
positions gives the same result, so no tie affected them.
