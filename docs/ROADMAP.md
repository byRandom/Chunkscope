# Road to 1.0

This is the plan from 0.3 to 1.0. Each version is a milestone that ships as
a release with binaries. Every item that generates something from a seed
follows the same rule as the rest of the project (see [PARITY.md](PARITY.md)):
read the game code with `javap`, port it, add an oracle fixture from the real
game, match it bit for bit, then break the port on purpose and check that a
test fails.

Target: Minecraft: Java Edition 26.3. If a new Minecraft version comes out
before 1.0, porting to it is a minor version of its own (see
"Following Minecraft" below).

## Where we are (0.3)

Done and verified against the game, for the Overworld:

- seeds, random sources, noise, climate and biomes;
- terrain: density, aquifers, heightmaps;
- structure placement, frequency rolls and exclusion zones;
- structure validation (biome and terrain) for every structure except
  mineshafts and ruined portals;
- strongholds; slime chunks;
- world spawn: the origin exactly; the spawn block as an estimate, measured
  against 32 real servers (the game's own spawn height is not deterministic);
- the desktop app (map, layers, inspector, seed finder) and release builds.

Not done: the Nether, the End, mineshafts, ruined portals, large-scale
verification, saved settings, signed builds, a stable command line.

## 0.4: the Nether, and ruined portals

The Nether uses the same machinery as the Overworld with other data.

1. **Nether biomes.** `MultiNoiseBiomeSource` with the `nether` preset: the
   oracle exports the Nether parameter list and its climate functions
   (temperature and vegetation only). The existing R-tree search, climate
   and fixtures are reused; a fixture records Nether biomes on many seeds
   and heights.
2. **Nether terrain.** The oracle exports the Nether `final_density` tree
   the same way as the Overworld one; Chunkscope evaluates it with the
   existing interpreter. No aquifers (a lava sea at y = 32). Fixture: base
   columns, as for the Overworld.
3. **Structures.**
   - Fortresses and bastions share the `nether_complexes` set (weighted
     selection, already ported). Fortresses start at the chunk corner,
     y = 64, with a biome check (simple). Bastions are jigsaw structures
     (ported; their start pool is exported like the others).
   - Nether fossils: a random height, then a scan of the base column for
     air above a solid block, below sea level. Needs the Nether terrain.
   - **Ruined portals**, all variants (on land, partly buried, underground,
     in a mountain, on the ocean floor, in the Nether): pick a setup by
     weight, then the template, rotation and mirror, then `findSuitableY`
     from the terrain at the template's corners. Mostly terrain sampling
     and random numbers, no piece generation.
4. **App.** A dimension switch (Overworld / Nether) with its own biome
   colors and layers; the seed finder can search Nether structures, measured
   from 0, 0.

Done when: Nether biomes, terrain and every Nether structure match the
game, and ruined portals are validated in both dimensions.

## 0.5: the End

1. **End biomes.** `TheEndBiomeSource`: the main island inside 64 chunks of
   the origin, else highlands, midlands, small islands or barrens by the
   value of the End island density at that point (thresholds 0.25, -0.0625,
   -0.21875).
2. **End terrain.** The island density uses `SimplexNoise`, a new noise to
   port (with fixtures of its own), inside the End `final_density`
   exported from the game.
3. **End cities.** A column check, a random rotation and the lowest corner
   of a 5×5 footprint, which must be high enough: needs the End terrain.
4. **End gateways** (optional for 1.0): the positions the game places
   gateways at after each dragon fight, if they come from the seed alone.
5. **App.** The End in the dimension switch; outer islands drawn from the
   terrain; end cities in the seed finder.

Done when: End biomes, the island terrain and end cities match the game.

## 0.6: mineshafts, and no "possible" left

1. **Mineshafts.** Their generation point is checked at the mineshaft's own
   height, which comes from generating its pieces (corridors, crossings,
   rooms, stairs) and then moving the whole structure (below sea level for
   normal mineshafts, inside a height range for badlands ones). Port
   `MineshaftPieces` far enough to get the final bounding box, with fixtures
   of whole piece lists from the game. This is the largest single port left.
2. **No "possible" markers.** With mineshafts done, every structure on the
   map and in the finder is validated. Remove the "possible" state from the
   app, or keep it only for data the game itself does not decide from the
   seed.
3. **Saved settings.** The app remembers the seed, view, open layers,
   dimension and the finder's last query (eframe persistence, a versioned
   settings file).

## 0.7: large-scale verification

The fixtures today hold thousands of samples on a few seeds. Before 1.0 the
port should be checked at a much larger scale.

1. **Cross-check against the game, not cubiomes.** The roadmap first named
   cubiomes as the large-scale cross-check, but cubiomes targets older
   versions with `double` noise: it cannot match 26.x worlds (see
   PARITY.md). The large-scale check therefore runs the game oracle itself
   in a "wide" mode: thousands of random seeds, sampled biomes in every
   dimension, every structure attempt in a large area, strongholds and
   spawn origins.
2. **Where it runs.** Too big for every pull request: a manual or weekly
   workflow (and before each release) that regenerates the wide fixtures,
   compares, and reports any difference with the seed and position.
3. **Fix what it finds**, add each case to the regular fixtures, and
   document any new pitfall in PARITY.md.

## 0.8: stable command line and formats

1. **A real command-line tool.** Turn the examples into one binary,
   `chunkscope-cli` (or `chunkscope` subcommands): `biome`, `height`,
   `structures`, `spawn`, `find`, with documented options and JSON output
   for scripts. Ship it in the release archives next to the app.
2. **Stable formats.** Freeze and document the settings file and the
   finder's query and result formats (import and export from the app), with
   a version number so later versions can read them.
3. **Signed builds.** Windows (Authenticode) and macOS (Developer ID and
   notarization) signing in the release workflow, so SmartScreen and
   Gatekeeper stop warning. **This needs the owner's decision and
   accounts:** an Apple Developer membership (yearly fee) for macOS, and a
   code-signing certificate or a free open-source signing service (for
   example SignPath) for Windows. Linux stays unsigned, with checksums.

## 1.0

Release candidates (`1.0.0-rc.N`) first, then 1.0 when:

- everything shown for the Overworld, the Nether and the End matches the
  game bit for bit and is verified by oracle tests, with no "possible"
  markers left, and the world spawn as far as the game itself is
  deterministic;
- the large-scale verification is clean;
- settings are saved; builds are signed;
- the command line and the saved formats are stable and documented;
- the README and a short user guide describe every feature.

## Following Minecraft

When a new Minecraft version changes world generation, supporting it is its
own minor version: pin the new version in `parity/game-oracle/run.sh`,
regenerate every fixture, see what breaks, and port the differences. Before
1.0 Chunkscope targets one version (26.3); supporting several versions at
once is a post-1.0 decision.

## After 1.0 (not planned yet)

Large biomes and amplified world presets, terrain shading on the map,
structure piece outlines, and more seed-finder filters (biome areas,
structure combinations, strongholds near the spawn).
