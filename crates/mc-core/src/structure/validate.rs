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
use crate::biome::{Biome, EndBiomeSource, NetherBiomeSource, OverworldBiomeSource};
use crate::random::{LegacyRandomSource, RandomSource, WorldgenRandom};
use crate::terrain::{EndTerrain, Heightmap, NetherTerrain, OverworldTerrain, Substance};

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

/// `VerticalAnchor`, resolved against the dimension's generation range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerticalAnchor {
    Absolute(i32),
    /// Offset above the lowest generated block.
    AboveBottom(i32),
    /// Offset below the highest generated block.
    BelowTop(i32),
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
    /// `NetherFortressStructure`: the chunk's corner at y = 64.
    NetherFortress,
    /// `NetherFossilStructure`: a random column and height, then down to
    /// the first air block above a solid one, above sea level.
    NetherFossil {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
    },
    /// `RuinedPortalStructure`, with its setups (one per placement it can
    /// take, chosen by weight).
    RuinedPortal(&'static [PortalSetup]),
    /// `WoodlandMansionStructure`
    WoodlandMansion,
    /// `EndCityStructure`: the same check as woodland mansions.
    EndCity,
    /// `OceanMonumentStructure`
    OceanMonument,
    /// Not ported yet (mineshafts).
    NotImplemented,
}

/// `RuinedPortalPiece.VerticalPlacement`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortalPlacement {
    OnLandSurface,
    PartlyBuried,
    OnOceanFloor,
    InMountain,
    Underground,
    InNether,
}

/// `RuinedPortalStructure.Setup`, the parts that decide where it goes.
#[derive(Clone, Copy, Debug)]
pub struct PortalSetup {
    pub placement: PortalPlacement,
    pub weight: f32,
    pub air_pocket_probability: f32,
}

impl PartialEq for PortalSetup {
    fn eq(&self, other: &Self) -> bool {
        self.placement == other.placement
            && self.weight.to_bits() == other.weight.to_bits()
            && self.air_pocket_probability.to_bits() == other.air_pocket_probability.to_bits()
    }
}

impl Eq for PortalSetup {}

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

/// A dimension's biome source, as structure validation reads it.
pub trait BiomeLookup {
    /// `getNoiseBiome(quartX, quartY, quartZ)`.
    fn biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome;
}

impl BiomeLookup for OverworldBiomeSource {
    fn biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome {
        OverworldBiomeSource::biome(self, quart_x, quart_y, quart_z)
    }
}

impl BiomeLookup for EndBiomeSource {
    fn biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome {
        EndBiomeSource::biome(self, quart_x, quart_y, quart_z)
    }
}

impl BiomeLookup for NetherBiomeSource {
    fn biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome {
        NetherBiomeSource::biome(self, quart_x, quart_y, quart_z)
    }
}

/// A dimension's terrain, as structure validation reads it.
#[derive(Clone, Copy)]
enum TerrainRef<'a> {
    Overworld(&'a OverworldTerrain),
    Nether(&'a NetherTerrain),
    End(&'a EndTerrain),
}

impl TerrainRef<'_> {
    fn base_height(self, x: i32, z: i32, heightmap: Heightmap) -> i32 {
        match self {
            TerrainRef::Overworld(t) => t.base_height(x, z, heightmap),
            TerrainRef::Nether(t) => t.base_height(x, z, heightmap),
            TerrainRef::End(t) => t.base_height(x, z, heightmap),
        }
    }

    /// The base column and its lowest y.
    fn base_column(self, x: i32, z: i32) -> (Vec<Substance>, i32) {
        match self {
            TerrainRef::Overworld(t) => (t.base_column(x, z), crate::terrain::MIN_Y),
            TerrainRef::Nether(t) => (t.base_column(x, z), crate::terrain::NETHER_MIN_Y),
            TerrainRef::End(t) => (t.base_column(x, z), crate::terrain::END_MIN_Y),
        }
    }
}

/// Validates structure attempts for one world seed and dimension.
pub struct StructureGenerator<'a> {
    seed: i64,
    biomes: &'a dyn BiomeLookup,
    /// The dimension's terrain, for the structures that need heights; `None`
    /// while it is not ported (they are then reported as unknown).
    terrain: Option<TerrainRef<'a>>,
    /// The level's lowest block and height (`LevelHeightAccessor`, from the
    /// dimension type).
    min_y: i32,
    height: i32,
    sea_level: i32,
    /// `WorldGenerationContext`: the generated range (the noise settings').
    gen_min_y: i32,
    gen_depth: i32,
}

impl<'a> StructureGenerator<'a> {
    /// The overworld.
    pub fn new(seed: i64, biomes: &'a OverworldBiomeSource, terrain: &'a OverworldTerrain) -> Self {
        Self {
            seed,
            biomes,
            terrain: Some(TerrainRef::Overworld(terrain)),
            min_y: crate::terrain::MIN_Y,
            height: crate::terrain::HEIGHT,
            sea_level: crate::terrain::SEA_LEVEL,
            gen_min_y: crate::terrain::MIN_Y,
            gen_depth: crate::terrain::HEIGHT,
        }
    }

    /// The Nether: a level from y 0 to 255 (`dimension_type/the_nether.json`)
    /// whose terrain is generated from y 0 to 127, with the lava sea at 32.
    pub fn nether(seed: i64, biomes: &'a NetherBiomeSource, terrain: &'a NetherTerrain) -> Self {
        Self {
            seed,
            biomes,
            terrain: Some(TerrainRef::Nether(terrain)),
            min_y: 0,
            height: 256,
            sea_level: crate::terrain::NETHER_SEA_LEVEL,
            gen_min_y: crate::terrain::NETHER_MIN_Y,
            gen_depth: crate::terrain::NETHER_HEIGHT,
        }
    }

    /// The End: a level from y 0 to 255 (`dimension_type/the_end.json`)
    /// whose terrain is generated from y 0 to 127, with sea level 0.
    pub fn end(seed: i64, biomes: &'a EndBiomeSource, terrain: &'a EndTerrain) -> Self {
        Self {
            seed,
            biomes,
            terrain: Some(TerrainRef::End(terrain)),
            min_y: 0,
            height: 256,
            sea_level: 0,
            gen_min_y: crate::terrain::END_MIN_Y,
            gen_depth: crate::terrain::END_HEIGHT,
        }
    }

    /// `VerticalAnchor.resolveY` in this dimension's `WorldGenerationContext`.
    fn resolve(&self, anchor: VerticalAnchor) -> i32 {
        match anchor {
            VerticalAnchor::Absolute(y) => y,
            VerticalAnchor::AboveBottom(offset) => self.gen_min_y + offset,
            VerticalAnchor::BelowTop(offset) => self.gen_depth - 1 + self.gen_min_y - offset,
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
        self.generation_point(structure, chunk_x, chunk_z, true)
    }

    /// `Structure.findGenerationPoint`, without the final biome check:
    /// where the structure would start if its biome allowed it.
    pub fn generation_point_any_biome(
        &self,
        structure: &'static Structure,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Option<Option<(i32, i32, i32)>> {
        self.generation_point(structure, chunk_x, chunk_z, false)
    }

    fn generation_point(
        &self,
        structure: &'static Structure,
        chunk_x: i32,
        chunk_z: i32,
        check_biome: bool,
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
        let needs_terrain = match structure.kind {
            StructureKind::NetherFortress | StructureKind::NotImplemented => false,
            StructureKind::Jigsaw {
                project_start_to_heightmap,
                ..
            } => project_start_to_heightmap.is_some(),
            _ => true,
        };
        if needs_terrain && self.terrain.is_none() {
            return None;
        }
        let point = match structure.kind {
            StructureKind::NetherFortress => Some((chunk_x << 4, 64, chunk_z << 4)),
            StructureKind::NetherFossil {
                min_inclusive,
                max_inclusive,
            } => ctx.nether_fossil(min_inclusive, max_inclusive),
            StructureKind::SinglePiece { width, depth } => ctx.single_piece(width, depth),
            StructureKind::OnTopOfChunkCenter(heightmap) => ctx.on_top_of_chunk_center(heightmap),
            StructureKind::RuinedPortal(setups) => ctx.ruined_portal(setups),
            StructureKind::WoodlandMansion | StructureKind::EndCity => ctx.lowest_5x5_point(),
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
        Some(point.filter(|&p| !check_biome || ctx.is_valid_biome(p)))
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
        let g = self.generator;
        self.could_structure_exist_in_column(x, z, g.min_y - 1, g.min_y + g.height - 1)
    }

    fn middle_x(&self) -> i32 {
        (self.chunk_x << 4) + 8
    }

    fn middle_z(&self) -> i32 {
        (self.chunk_z << 4) + 8
    }

    /// The terrain; only called for structures that need it, which
    /// `try_structure` checks first.
    fn terrain(&self) -> TerrainRef<'_> {
        self.generator.terrain.expect("checked by try_structure")
    }

    /// `UniformHeight.sample`: `Mth.randomBetweenInclusive`, or the minimum
    /// for a reversed range.
    fn uniform(&mut self, min: i32, max: i32) -> i32 {
        if min > max {
            min
        } else {
            self.random.next_int_bounded(max - min + 1) + min
        }
    }

    /// `NetherFossilStructure.findGenerationPoint`.
    fn nether_fossil(
        &mut self,
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
    ) -> Option<(i32, i32, i32)> {
        let x = (self.chunk_x << 4) + self.random.next_int_bounded(16);
        let z = (self.chunk_z << 4) + self.random.next_int_bounded(16);
        let sea = self.generator.sea_level;
        let (min, max) = (
            self.generator.resolve(min_inclusive),
            self.generator.resolve(max_inclusive),
        );
        let mut y = self.uniform(min, max);
        let (column, bottom) = self.terrain().base_column(x, z);
        let block = |y: i32| {
            usize::try_from(y - bottom)
                .ok()
                .and_then(|i| column.get(i).copied())
                .unwrap_or(Substance::Air)
        };
        // Down to the first air block standing on a block with a sturdy top
        // (or soul sand, which the base column never has).
        while y > sea {
            let here = block(y);
            y -= 1;
            if here == Substance::Air && block(y) == Substance::Solid {
                break;
            }
        }
        (y > sea).then_some((x, y, z))
    }

    /// `RuinedPortalStructure.findGenerationPoint`: a setup by weight, then
    /// the template, rotation and mirror, and a height from
    /// [`Self::portal_suitable_y`] at the chunk's corner.
    fn ruined_portal(&mut self, setups: &[PortalSetup]) -> Option<(i32, i32, i32)> {
        let setup = if setups.len() > 1 {
            let total = setups.iter().fold(0.0f32, |sum, s| sum + s.weight);
            let mut f = self.random.next_float();
            // The game throws if rounding leaves nothing picked.
            *setups.iter().find(|s| {
                f -= s.weight / total;
                f < 0.0
            })?
        } else {
            setups[0]
        };
        // `sample`: 0 and 1 draw nothing.
        let p = setup.air_pocket_probability;
        let air_pocket = if p == 0.0 {
            false
        } else if p == 1.0 {
            true
        } else {
            self.random.next_float() < p
        };
        let templates: &[[i32; 3]] = if self.random.next_float() < 0.05 {
            &super::generated::GIANT_RUINED_PORTALS
        } else {
            &super::generated::RUINED_PORTALS
        };
        let [sx, sy, sz] = templates[self.random.next_int_bounded(templates.len() as i32) as usize];
        let rotation = self.random.next_int_bounded(4);
        let mirror = self.random.next_float() >= 0.5; // FRONT_BACK
        // StructureTemplate.getBoundingBox at the chunk's corner, pivoting
        // around the template's center.
        let (ox, oz) = (self.chunk_x << 4, self.chunk_z << 4);
        let (px, pz) = (sx / 2, sz / 2);
        let transform = |x: i32, z: i32| {
            let x = if mirror { -x } else { x };
            match rotation {
                1 => (px + pz - z, pz - px + x), // CLOCKWISE_90
                2 => (px + px - x, pz + pz - z), // CLOCKWISE_180
                3 => (px - pz + z, px + pz - x), // COUNTERCLOCKWISE_90
                _ => (x, z),
            }
        };
        let (a, b) = (transform(0, 0), transform(sx - 1, sz - 1));
        let bbox = [
            ox + a.0.min(b.0),
            oz + a.1.min(b.1),
            ox + a.0.max(b.0),
            oz + a.1.max(b.1),
        ];
        let center = (
            bbox[0] + (bbox[2] - bbox[0] + 1) / 2,
            bbox[1] + (bbox[3] - bbox[1] + 1) / 2,
        );
        let heightmap = if setup.placement == PortalPlacement::OnOceanFloor {
            Heightmap::OceanFloor
        } else {
            Heightmap::WorldSurface
        };
        let surface = self.first_occupied_height(center.0, center.1, heightmap);
        let y = self.portal_suitable_y(setup.placement, air_pocket, surface, sy, bbox, heightmap);
        Some((ox, y, oz))
    }

    /// `RuinedPortalStructure.findSuitableY`: a starting height for the
    /// placement, then down to the first height where at least three of
    /// the bounding box's corners are in the ground.
    fn portal_suitable_y(
        &mut self,
        placement: PortalPlacement,
        air_pocket: bool,
        surface: i32,
        y_span: i32,
        [min_x, min_z, max_x, max_z]: [i32; 4],
        heightmap: Heightmap,
    ) -> i32 {
        let min_y = self.generator.min_y + 15;
        // `getRandomWithinInterval`: the upper end for an empty interval.
        let within = |ctx: &mut Self, lo: i32, hi: i32| {
            if lo < hi { ctx.uniform(lo, hi) } else { hi }
        };
        let start = match placement {
            PortalPlacement::InNether if air_pocket => self.uniform(32, 100),
            PortalPlacement::InNether if self.random.next_float() < 0.5 => self.uniform(27, 29),
            PortalPlacement::InNether => self.uniform(29, 100),
            PortalPlacement::InMountain => within(self, 70, surface - y_span),
            PortalPlacement::Underground => within(self, min_y, surface - y_span),
            PortalPlacement::PartlyBuried => surface - y_span + self.uniform(2, 8),
            PortalPlacement::OnLandSurface | PortalPlacement::OnOceanFloor => surface,
        };
        let columns: Vec<(Vec<Substance>, i32)> = [
            (min_x, min_z),
            (max_x, min_z),
            (min_x, max_z),
            (max_x, max_z),
        ]
        .iter()
        .map(|&(x, z)| self.terrain().base_column(x, z))
        .collect();
        let opaque = |s: Substance| match heightmap {
            Heightmap::WorldSurface => s.is_world_surface(),
            Heightmap::OceanFloor => s.is_ocean_floor(),
        };
        let mut y = start;
        while y > min_y {
            let solid = columns
                .iter()
                .filter(|(column, bottom)| {
                    // NoiseColumn.getBlock: air outside the column.
                    let block = usize::try_from(y - bottom)
                        .ok()
                        .and_then(|i| column.get(i).copied())
                        .unwrap_or(Substance::Air);
                    opaque(block)
                })
                .count();
            if solid >= 3 {
                break;
            }
            y -= 1;
        }
        y
    }

    /// `ChunkGenerator.getFirstOccupiedHeight`
    fn first_occupied_height(&self, x: i32, z: i32, heightmap: Heightmap) -> i32 {
        self.terrain().base_height(x, z, heightmap) - 1
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
        if self.lowest_y(self.chunk_x << 4, self.chunk_z << 4, width, depth)
            < self.generator.sea_level
        {
            return None;
        }
        Some(self.on_top_without_biome_check(Heightmap::WorldSurface))
    }

    /// `WoodlandMansionStructure` and `EndCityStructure`
    /// `.findGenerationPoint`: a random rotation, then the lowest corner of a
    /// 5 x 5 footprint from the chunk's (7, 7), which must be at y 60 or more.
    fn lowest_5x5_point(&mut self) -> Option<(i32, i32, i32)> {
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
            } => self.uniform(min_inclusive, max_inclusive),
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
                let g = self.generator;
                let max = g.min_y + g.height - 1;
                if !self.could_structure_exist_in_column(center_x, center_z, g.min_y, max) {
                    return None;
                }
                start_y + self.terrain().base_height(center_x, center_z, heightmap)
            }
        };
        // The start piece is moved so its ground level sits at `y`.
        let dy = y - (min_y + element.ground_level_delta);
        if let Some((bottom, top)) = padding {
            let g = self.generator;
            let max = g.min_y + g.height - 1;
            if min_y + dy < g.min_y + bottom || max_y + dy > max - top {
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
                let sea = self.generator.sea_level;
                for qy in quart(sea - r)..=quart(sea + r) {
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
