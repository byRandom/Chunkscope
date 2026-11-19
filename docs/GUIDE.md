# User guide

Chunkscope shows what a Minecraft: Java Edition seed generates, without
opening the game: the biomes, where the structures are, the slime chunks and
the world spawn, in the Overworld, the Nether and the End. It can also search
thousands of seeds for the one you want. Everything runs on your computer.

It matches Minecraft: Java Edition 26.3 (the version is in the About tab).
Bedrock Edition is not supported.

## Start

Unpack the archive from the [releases
page](https://github.com/byRandom/Chunkscope/releases) and run `chunkscope`
(`chunkscope.exe` on Windows). The binaries are not signed, so Windows
SmartScreen and macOS ask for confirmation; the [README](../README.md#download)
says how to get past it. The archive also holds `chunkscope-cli`, the command
line tool (see [CLI.md](CLI.md)).

The app remembers your seed, dimension, view, layers and the finder's query
between runs ([FORMATS.md](FORMATS.md) says what is saved).

## The map

The **Map** tab is a world map you drag and zoom.

- **Drag** to pan, **scroll** to zoom (or use the zoom buttons on the map).
  The home button goes back to 0, 0.
- **Hover** to see the block, chunk, region, biome and whether it is a slime
  chunk in the Inspector on the right.
- **Click a marker** to select it. The Inspector shows where the structure
  starts and what it is, and has **Copy /tp** (a `/tp @s x ~ z` command) and
  **Center**.
- **Right-click** the map to copy the position or a `/tp` command, or to
  center there.
- **Go to** in the Inspector jumps to coordinates.

### World

- **Seed.** A number, or any text: like the game's world creation screen,
  text is turned into a number (the app shows which). The buttons copy the
  numeric seed and pick a random one. A blank seed is ignored (the game would
  pick a random world).
- **Dimension.** Overworld, Nether or End. Each has its own biomes, terrain
  and structures.
- **Biome Y.** The height the biomes are looked up at (they change with
  height in caves and in the Nether).

### Layers

Biomes, slime chunks (Overworld) and the grid can be switched on and off,
and each structure has its own layer (the **Show all** / **Hide all** button
does them all). Layers too dense to read at the current zoom are not drawn
and say from which zoom they appear. Colors for every biome are listed in the
Inspector.

What the markers mean:

- Every structure marker is **checked like the game does**: its biome, and
  the terrain where the structure needs it. If it is on the map, the
  structure generates there (a ruined portal, a mineshaft, a village in the
  right biome...). Strongholds are exact too.
- The **world spawn** is an estimate. The point the game searches around is
  exact, but the block you really appear on also depends on trees and
  lakes that only exist once the chunks are generated.
- In the End, a green dot marks end cities **with a ship** (an elytra).
  The outer islands are drawn from their density: their edges are
  approximate, their biomes and structures are exact. End gateways are not
  shown yet (planned before 1.0).

## The finder

The **Finder** tab searches seeds for what you describe, and shows every
seed that matches. Click one to open it on the map.

You describe a search as a **query**. Fill the form, or write the text at the
top (the two follow each other):

```text
center=spawn villages:300:2 pillager_outposts:500 biome=plains,meadow
```

- **Center:** distances are measured from the world spawn, or from 0, 0.
- **Structures:** "at least *n* of this structure within *r* blocks". **+ Add structure** adds as
  many as you like. Nether and End structures are measured from 0, 0 in their
  dimension, whatever the center. Tick **With a ship (elytra)** to keep only
  end cities with a ship.
- **Biome at the center:** the biome at the center must be one of those you
  pick.
- **Seeds:** where to start and how many to check, or **random seeds** (the
  start becomes a salt: the same salt gives the same seeds).

**Search** starts, **Stop** stops. Matches appear as they are found, and the
copy button next to **Results** copies all of them, one line per seed. Every
match is exact: structures are checked against biome and terrain like on the
map. A search runs on all your processor's threads; a village within 300
blocks of the spawn finds hundreds of seeds per second, a rare combination
may need millions of seeds.

Tips: a copied query pastes back into the finder and into the command line.
A text that does not parse is explained under the field.

## The command line

`chunkscope-cli` gives the same answers in a terminal or a script, with JSON
if you want it:

```bash
chunkscope-cli biome hello 100 -300
chunkscope-cli structures hello 0 0 800 --json
chunkscope-cli find --count 100000 villages:300 pillager_outposts:500
```

[CLI.md](CLI.md) lists every command, and [FORMATS.md](FORMATS.md) what its
output promises.

## What Chunkscope does not do

- Bedrock Edition, other Minecraft versions than the one shown in About,
  data packs or mods that change world generation, and large biomes or
  amplified worlds (planned after 1.0, see [ROADMAP.md](ROADMAP.md)).
- Show terrain shading, caves or the shape of structures: the map shows
  biomes and where structures start.
- Know the exact spawn block (see above).

## If something is different in the game

Chunkscope's promise is that it matches the game. If a seed, position and
structure disagree with what you see in Minecraft 26.3, please open an issue
on [GitHub](https://github.com/byRandom/Chunkscope/issues) with the seed,
the dimension and the position. [PARITY.md](PARITY.md) explains how every
part is verified.
