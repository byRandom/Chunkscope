# Formats

What Chunkscope saves, prints and accepts, and what it promises about each.
The promise holds from 1.0; before that the same rules apply, and a break
is called out in the release notes.

## The query (text)

What the finder searches for. The app shows, copies and pastes it; the
command line's `find` takes it as its arguments (see [CLI.md](CLI.md)).

```text
center=spawn|zero  <target>[+ship]:<radius>[:<count>]  biome=<biome>,...
```

Words are separated by spaces, in any order.

- `center=spawn` (default) or `center=zero`: where distances are measured
  from. Nether and End targets are always measured from 0, 0 in their
  dimension.
- `<target>:<radius>[:<count>]`: at least `count` (default 1) structures of a
  target within `radius` blocks (0 to 30,000,000), measured to the position
  `/locate` reports. `+ship` keeps only end cities with a ship.
- `biome=<biome>,...`: the biome at the center is one of these. Names are
  the game's, with or without `minecraft:`.
- An empty query is valid (it matches every seed).
- Printing a query gives its canonical form (defaults left out), and
  parsing that gives the same query back.

Targets are named by id: `villages`, `woodland_mansions`, `ocean_monuments`,
`pillager_outposts`, `desert_pyramids`, `jungle_temples`, `swamp_huts`,
`igloos`, `ancient_cities`, `trial_chambers`, `trail_ruins`,
`abandoned_camps`, `ruined_portals`, `shipwrecks`, `ocean_ruins`,
`buried_treasures`, `mineshafts`, `fortresses`, `bastions`, `nether_fossils`,
`nether_ruined_portals`, `end_cities` (`chunkscope-cli targets` lists them).

**Promise:** a query that parses keeps parsing, with the same meaning. New
targets, words and options may be added; ids are never renamed or removed
(a test pins them). A query using something newer than your version fails
with an error naming it, never with a different meaning.

## The result line

One line per matching seed, printed by `find` and copied by the app:

```text
<seed>  center <x>, <z>  <biome>  <found>; <found>; ...
```

where each `<found>` is `<structure> <x>, <z> (<distance>)`, with
`(<dimension>)` after the structure for the Nether and the End. For
example: `12  center -13, -38  plains  village_plains 160, 32 (187)`.

**Promise:** it is meant to be read by people and pasted into notes. A later
version may add information at the end of a line. Scripts should use
`--json` instead.

## JSON (command line)

Documented in [CLI.md](CLI.md#json). **Promise:** within a major version,
fields are only added, never renamed, removed or changed in type; read the
ones you need and ignore the rest.

## The settings file

The app saves what it remembers between runs (seed, dimension, view,
layers, the finder's query) as a [RON](https://github.com/ron-rs/ron) text
file, `app.ron`, in the platform's app data folder, under the key
`chunkscope-settings`. Deleting the file resets everything.

The fields are those of `Settings` in
`crates/chunkscope-app/src/settings.rs`; a test pins the whole saved form,
so a change to it cannot go unnoticed. The file starts with `version`
(currently **1**).

**Promise:**

- A file written by an older version always loads. Fields it lacks take
  their default.
- A field this version does not know is ignored.
- Layers and finder filters are saved by id, never by position, so adding or
  reordering them keeps your choices. A layer that no longer exists is
  ignored.
- New fields do not change the version. The version is bumped only when an
  existing field changes meaning (and then the old meaning is converted when
  it is read).
- A file with a **higher** version than the app knows (you went back to an
  older Chunkscope) is not read, because its fields may mean something else:
  the app starts with the defaults, and saves its own on exit.
