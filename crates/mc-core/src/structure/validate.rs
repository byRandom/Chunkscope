//! Structure validation (Minecraft 26.x): whether a structure set's attempt
//! at a chunk really generates a structure, which one, and where.
//!
//! A random-spread set *attempts* a structure at one chunk per region (see
//! [`super::placement`]). The game then picks one of the set's structures
//! and asks it for a generation point (`Structure.findGenerationPoint`);
//! the structure generates when that point exists and its biome is one of
//! the structure's biomes. Each structure type finds its point differently:
//! on top of the chunk center, at the lowest corner of its footprint, or
//! (jigsaw structures) at the center of their start piece.

use std::sync::OnceLock;

use super::placement::{StructurePlacement, StructureSet};
use crate::biome::{Biome, OverworldBiomeSource};
use crate::random::{LegacyRandomSource, RandomSource, WorldgenRandom};
use crate::terrain::{Heightmap, MIN_Y, OverworldTerrain, SEA_LEVEL};

/// `HeightProvider` of a jigsaw structure's start, with absolute anchors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeightProvider {
    /// `ConstantHeight`
    Constant(i32),
    /// `UniformHeight`
    Uniform {
        min_inclusive: i32,
        max_inclusive: i32,
    },
}

/// How a structure type finds its generation point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructureKind {
    /// `JigsawStructure` (villages, outposts, ancient cities...).
    Jigsaw {
        start_pool: &'static str,
        start_jigsaw_name: Option<&'static str>,
        start_height: HeightProvider,
        project_start_to_heightmap: Option<Heightmap>,
        /// `DimensionPadding` (bottom, top); `None` for the default, which the
        /// game does not check at all.
        dimension_padding: Option<(i32, i32)>,
    },
    /// `SinglePieceStructure` (desert pyramids, jungle temples): the lowest
    /// corner of the footprint must be at sea level or above.
    SinglePiece { width: i32, depth: i32 },
    /// `Structure.onTopOfChunkCenter` with this heightmap (swamp huts,
    /// igloos, ocean ruins, shipwrecks, buried treasure).
    OnTopOfChunkCenter(Heightmap),
    /// `WoodlandMansionStructure`
    WoodlandMansion,
    /// `OceanMonumentStructure`
    OceanMonument,
    /// Not ported yet (mineshafts, ruined portals).
    NotImplemented,
}

/// Where a start piece is anchored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    /// At the start position (no `start_jigsaw_name`).
    Origin,
    /// At the named jigsaw block, per rotation, relative to the start position.
    Jigsaw([[i32; 3]; 4]),
    /// The element has no jigsaw block with the structure's start name.
    Missing,
}

/// A start pool element: its bounding box at the origin for each rotation
/// (`NONE`, `CLOCKWISE_90`, `CLOCKWISE_180`, `COUNTERCLOCKWISE_90`) as
/// `[min_x, min_y, min_z, max_x, max_y, max_z]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolElement {
    pub boxes: [[i32; 6]; 4],
    pub ground_level_delta: i32,
    pub anchor: Anchor,
}

/// A jigsaw structure's start pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartPool {
    pub name: &'static str,
    /// Distinct elements; `None` is the empty element.
    pub elements: &'static [Option<PoolElement>],
    /// The game's weighted element list, as indices into `elements`.
    pub templates: &'static [u16],
}

/// A vanilla structure, as the structure sets refer to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Structure {
    /// Registry name, e.g. `minecraft:village_plains`.
    pub name: &'static str,
    /// The structure's `biomes` tag, resolved.
    pub biomes: &'static [&'static str],
    pub kind: StructureKind,
}

impl StartPool {
    pub fn find(name: &str) -> Option<&'static StartPool> {
        super::generated::START_POOLS
            .iter()
            .find(|p| p.name == name)
    }
}

impl Structure {
    pub fn find(name: &str) -> Option<&'static Structure> {
        super::generated::STRUCTURES.iter().find(|s| s.name == name)
    }

    fn index(&self) -> usize {
        super::generated::STRUCTURES
            .iter()
            .position(|s| s.name == self.name)
            .expect("structure from the table")
    }
}

/// One structure the game tried at a chunk, and its generation point if it
/// generates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attempt {
    pub structure: &'static Structure,
    /// The validated generation point (block position), or `None` if the
    /// structure does not generate.
    pub point: Option<(i32, i32, i32)>,
}

/// What a structure set's attempt at a chunk gives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The structures tried, in the game's order; the last one generates.
    Generates(Vec<Attempt>),
    /// Every structure was tried and none generates.
    Nothing(Vec<Attempt>),
    /// A structure that is not ported yet came up, so the result is unknown.
    /// Holds the attempts before it.
    Unknown(Vec<Attempt>),
}

impl Outcome {
    pub fn attempts(&self) -> &[Attempt] {
        match self {
            Outcome::Generates(a) | Outcome::Nothing(a) | Outcome::Unknown(a) => a,
        }
    }

    /// The structure that generates and its generation point.
    pub fn generated(&self) -> Option<(&'static Structure, (i32, i32, i32))> {
        match self {
            Outcome::Generates(a) => a.last().and_then(|a| a.point.map(|p| (a.structure, p))),
            _ => None,
        }
    }
}

/// Biome membership tables, built once.
struct BiomeTables {
    /// Per structure (index into `STRUCTURES`), per biome id.
    structures: Vec<Vec<bool>>,
    monument_surrounding: Vec<bool>,
}

fn tables() -> &'static BiomeTables {
    static TABLES: OnceLock<BiomeTables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let set = |names: &[&str]| {
            let mut v = vec![false; Biome::all().count()];
            for n in names {
                if let Some(b) = Biome::from_name(n) {
                    v[b.0 as usize] = true;
                }
            }
            v
        };
        BiomeTables {
            structures: super::generated::STRUCTURES
                .iter()
                .map(|s| set(s.biomes))
                .collect(),
            monument_surrounding: set(super::generated::OCEAN_MONUMENT_SURROUNDING),
        }
    })
}

/// `QuartPos.fromBlock`
fn quart(block: i32) -> i32 {
    block >> 2
}

/// Validates structure attempts for one world seed.
pub struct StructureGenerator<'a> {
    seed: i64,
    biomes: &'a OverworldBiomeSource,
    terrain: &'a OverworldTerrain,
}

impl<'a> StructureGenerator<'a> {
    pub fn new(seed: i64, biomes: &'a OverworldBiomeSource, terrain: &'a OverworldTerrain) -> Self {
        Self {
            seed,
            biomes,
            terrain,
        }
    }

    /// `ChunkGenerator.createStructures` for one set at a chunk where the set
    /// attempts a structure (see [`StructureSet::is_structure_chunk`]).
    pub fn attempt(&self, set: &StructureSet, chunk_x: i32, chunk_z: i32) -> Outcome {
        let mut attempts = Vec::new();
        let StructurePlacement::RandomSpread(_) = set.placement else {
            return Outcome::Unknown(attempts);
        };
        let mut entries: Vec<(&'static Structure, i32)> = set
            .structures
            .iter()
            .map(|&(name, weight)| (Structure::find(name).expect("structure of a set"), weight))
            .collect();
        if entries.len() == 1 {
            let structure = entries[0].0;
            let Some(point) = self.try_structure(structure, chunk_x, chunk_z) else {
                return Outcome::Unknown(attempts);
            };
            attempts.push(Attempt { structure, point });
            return if point.is_some() {
                Outcome::Generates(attempts)
            } else {
                Outcome::Nothing(attempts)
            };
        }
        let mut random = WorldgenRandom::new(LegacyRandomSource::new(0));
        random.set_large_feature_seed(self.seed, chunk_x, chunk_z);
        let mut total: i32 = entries.iter().map(|e| e.1).sum();
        while !entries.is_empty() {
            let mut pick = random.next_int_bounded(total);
            let mut i = 0;
            for e in &entries {
                pick -= e.1;
                if pick < 0 {
                    break;
                }
                i += 1;
            }
            let (structure, weight) = entries[i];
            match self.try_structure(structure, chunk_x, chunk_z) {
                None => {
                    return Outcome::Unknown(attempts);
                }
                Some(point) => {
                    attempts.push(Attempt { structure, point });
                    if point.is_some() {
                        return Outcome::Generates(attempts);
                    }
                }
            }
            entries.remove(i);
            total -= weight;
        }
        Outcome::Nothing(attempts)
    }

    /// `Structure.findValidGenerationPoint`: `Some(None)` when the structure
    /// does not generate, `None` when its type is not ported yet.
    pub fn try_structure(
        &self,
        structure: &'static Structure,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Option<Option<(i32, i32, i32)>> {
        let valid = &tables().structures[structure.index()];
        let mut ctx = Context {
            generator: self,
            valid,
            chunk_x,
            chunk_z,
            random: {
                let mut r = WorldgenRandom::new(LegacyRandomSource::new(0));
                r.set_large_feature_seed(self.seed, chunk_x, chunk_z);
                r
            },
        };
        let point = match structure.kind {
            StructureKind::SinglePiece { width, depth } => ctx.single_piece(width, depth),
            StructureKind::OnTopOfChunkCenter(heightmap) => ctx.on_top_of_chunk_center(heightmap),
            StructureKind::WoodlandMansion => ctx.woodland_mansion(),
            StructureKind::OceanMonument => ctx.ocean_monument(),
            StructureKind::Jigsaw {
                start_pool,
                start_jigsaw_name: _,
                start_height,
                project_start_to_heightmap,
                dimension_padding,
            } => ctx.jigsaw(
                start_pool,
                start_height,
                project_start_to_heightmap,
                dimension_padding,
            ),
            StructureKind::NotImplemented => return None,
        };
        Some(point.filter(|&p| ctx.is_valid_biome(p)))
    }
}

/// `Structure.GenerationContext`
struct Context<'g, 'a> {
    generator: &'g StructureGenerator<'a>,
    valid: &'static [bool],
    chunk_x: i32,
    chunk_z: i32,
    random: WorldgenRandom<LegacyRandomSource>,
}

impl Context<'_, '_> {
    fn biome_ok(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> bool {
        let b = self.generator.biomes.biome(quart_x, quart_y, quart_z);
        self.valid[b.0 as usize]
    }

    /// `isValidBiome`: the biome at the generation point.
    fn is_valid_biome(&self, (x, y, z): (i32, i32, i32)) -> bool {
        self.biome_ok(quart(x), quart(y), quart(z))
    }

    /// `couldStructureExistInColumn`: any valid biome from `min_y` to `max_y`.
    fn could_structure_exist_in_column(&self, x: i32, z: i32, min_y: i32, max_y: i32) -> bool {
        let (qx, qz) = (quart(x), quart(z));
        (quart(min_y)..=quart(max_y)).any(|qy| self.biome_ok(qx, qy, qz))
    }

    /// `couldValidBiomeExistInTerrainColumn`
    fn could_valid_biome_exist_in_terrain_column(&self, x: i32, z: i32) -> bool {
        self.could_structure_exist_in_column(x, z, MIN_Y - 1, MIN_Y + crate::terrain::HEIGHT - 1)
    }

    fn middle_x(&self) -> i32 {
        (self.chunk_x << 4) + 8
    }

    fn middle_z(&self) -> i32 {
        (self.chunk_z << 4) + 8
    }

    /// `ChunkGenerator.getFirstOccupiedHeight`
    fn first_occupied_height(&self, x: i32, z: i32, heightmap: Heightmap) -> i32 {
        self.generator.terrain.base_height(x, z, heightmap) - 1
    }

    /// `Structure.getLowestY(context, x, z, width, depth)`: the lowest
    /// `WORLD_SURFACE_WG` height of the four corners.
    fn lowest_y(&self, x: i32, z: i32, width: i32, depth: i32) -> i32 {
        let h = |x, z| self.first_occupied_height(x, z, Heightmap::WorldSurface);
        let corners = [
            h(x, z),
            h(x, z + depth),
            h(x + width, z),
            h(x + width, z + depth),
        ];
        corners[0].min(corners[1]).min(corners[2].min(corners[3]))
    }

    /// `Structure.onTopOfChunkCenterWithoutBiomeCheck`
    fn on_top_without_biome_check(&self, heightmap: Heightmap) -> (i32, i32, i32) {
        let (x, z) = (self.middle_x(), self.middle_z());
        (x, self.first_occupied_height(x, z, heightmap), z)
    }

    /// `Structure.onTopOfChunkCenter`
    fn on_top_of_chunk_center(&self, heightmap: Heightmap) -> Option<(i32, i32, i32)> {
        if !self.could_valid_biome_exist_in_terrain_column(self.middle_x(), self.middle_z()) {
            return None;
        }
        Some(self.on_top_without_biome_check(heightmap))
    }

    /// `SinglePieceStructure.findGenerationPoint`
    fn single_piece(&self, width: i32, depth: i32) -> Option<(i32, i32, i32)> {
        if !self.could_valid_biome_exist_in_terrain_column(self.middle_x(), self.middle_z()) {
            return None;
        }
        if self.lowest_y(self.chunk_x << 4, self.chunk_z << 4, width, depth) < SEA_LEVEL {
            return None;
        }
        Some(self.on_top_without_biome_check(Heightmap::WorldSurface))
    }

    /// `WoodlandMansionStructure.findGenerationPoint`
    fn woodland_mansion(&mut self) -> Option<(i32, i32, i32)> {
        let (x, z) = ((self.chunk_x << 4) + 7, (self.chunk_z << 4) + 7);
        if !self.could_valid_biome_exist_in_terrain_column(x, z) {
            return None;
        }
        // Rotation.getRandom; then Structure.getLowestYIn5by5Box.
        let (dx, dz) = match self.random.next_int_bounded(4) {
            0 => (5, 5),   // NONE
            1 => (-5, 5),  // CLOCKWISE_90
            2 => (-5, -5), // CLOCKWISE_180
            _ => (5, -5),  // COUNTERCLOCKWISE_90
        };
        let y = self.lowest_y(x, z, dx, dz);
        (y >= 60).then_some((x, y, z))
    }

    /// `JigsawStructure.findGenerationPoint` and the start of
    /// `JigsawPlacement.addPieces`: the center of the start piece, on the
    /// terrain if the structure projects its start to a heightmap.
    fn jigsaw(
        &mut self,
        start_pool: &str,
        start_height: HeightProvider,
        project: Option<Heightmap>,
        padding: Option<(i32, i32)>,
    ) -> Option<(i32, i32, i32)> {
        let start_y = match start_height {
            HeightProvider::Constant(y) => y,
            // UniformHeight.sample: Mth.randomBetweenInclusive (a reversed range
            // gives its minimum).
            HeightProvider::Uniform {
                min_inclusive,
                max_inclusive,
            } => {
                if min_inclusive > max_inclusive {
                    min_inclusive
                } else {
                    self.random
                        .next_int_bounded(max_inclusive - min_inclusive + 1)
                        + min_inclusive
                }
            }
        };
        let (x0, z0) = (self.chunk_x << 4, self.chunk_z << 4);
        let rotation = self.random.next_int_bounded(4) as usize;
        let pool = StartPool::find(start_pool).expect("start pool from the table");
        if pool.templates.is_empty() {
            return None; // EmptyPoolElement
        }
        let index =
            pool.templates[self.random.next_int_bounded(pool.templates.len() as i32) as usize];
        let element = pool.elements[index as usize]?;
        let offset = match element.anchor {
            Anchor::Origin => [0, 0, 0],
            // The game shuffles the jigsaw blocks first (with this random), but
            // the oracle checked the name is unique in every start element.
            Anchor::Jigsaw(anchors) => anchors[rotation],
            Anchor::Missing => return None,
        };
        let piece = [x0 - offset[0], start_y - offset[1], z0 - offset[2]];
        let b = element.boxes[rotation];
        let (min_x, min_y, min_z) = (b[0] + piece[0], b[1] + piece[1], b[2] + piece[2]);
        let (max_x, max_y, max_z) = (b[3] + piece[0], b[4] + piece[1], b[5] + piece[2]);
        // Java int division (truncates toward zero, like Rust's).
        let center_x = (max_x + min_x) / 2;
        let center_z = (max_z + min_z) / 2;
        let y = match project {
            None => piece[1],
            Some(heightmap) => {
                let max = MIN_Y + crate::terrain::HEIGHT - 1;
                if !self.could_structure_exist_in_column(center_x, center_z, MIN_Y, max) {
                    return None;
                }
                start_y
                    + self
                        .generator
                        .terrain
                        .base_height(center_x, center_z, heightmap)
            }
        };
        // The start piece is moved so its ground level sits at `y`.
        let dy = y - (min_y + element.ground_level_delta);
        if let Some((bottom, top)) = padding {
            let max = MIN_Y + crate::terrain::HEIGHT - 1;
            if min_y + dy < MIN_Y + bottom || max_y + dy > max - top {
                return None;
            }
        }
        Some((center_x, y + offset[1], center_z))
    }

    /// `OceanMonumentStructure.findGenerationPoint`
    fn ocean_monument(&self) -> Option<(i32, i32, i32)> {
        let (x, z) = ((self.chunk_x << 4) + 9, (self.chunk_z << 4) + 9);
        // BiomeResolver.getBiomesWithin(x, seaLevel, z, 29): z, then x, then y.
        // The game collects every biome before checking them, so all of the
        // lookups happen (they move the biome search's starting point).
        let r = 29;
        let surrounding = &tables().monument_surrounding;
        let mut all_surrounding = true;
        for qz in quart(z - r)..=quart(z + r) {
            for qx in quart(x - r)..=quart(x + r) {
                for qy in quart(SEA_LEVEL - r)..=quart(SEA_LEVEL + r) {
                    let b = self.generator.biomes.biome(qx, qy, qz);
                    all_surrounding &= surrounding[b.0 as usize];
                }
            }
        }
        if !all_surrounding {
            return None;
        }
        self.on_top_of_chunk_center(Heightmap::OceanFloor)
    }
}
