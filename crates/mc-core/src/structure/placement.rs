//! Structure placement (`levelgen.structure.placement`): which chunks a
//! structure set *tries* to generate in. This is pure RNG; whether the
//! structure then fits there (biome, terrain) is decided later by the game.

use crate::random::{LegacyRandomSource, RandomSource, WorldgenRandom};

/// `RandomSpreadType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpreadType {
    /// `nextInt(n)`
    Linear,
    /// `(nextInt(n) + nextInt(n)) / 2`: biased towards the region center.
    Triangular,
}

impl SpreadType {
    /// `RandomSpreadType.evaluate`.
    pub fn evaluate(self, random: &mut impl RandomSource, limit: i32) -> i32 {
        match self {
            SpreadType::Linear => random.next_int_bounded(limit),
            SpreadType::Triangular => {
                let a = random.next_int_bounded(limit);
                let b = random.next_int_bounded(limit);
                a.wrapping_add(b) / 2
            }
        }
    }
}

/// `AbstractSpreadingStructurePlacement.FrequencyReductionMethod`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FrequencyReduction {
    /// `default` (`probabilityReducer`).
    Default,
    /// `legacy_type_1` (`legacyPillagerOutpostReducer`).
    LegacyType1,
    /// `legacy_type_2` (`legacyArbitrarySaltProbabilityReducer`).
    LegacyType2,
    /// `legacy_type_3` (`legacyProbabilityReducerWithDouble`).
    LegacyType3,
}

fn legacy_worldgen_random() -> WorldgenRandom<LegacyRandomSource> {
    WorldgenRandom::new(LegacyRandomSource::new(0))
}

impl FrequencyReduction {
    /// `FrequencyReductionMethod.shouldGenerate(seed, salt, chunkX, chunkZ, frequency)`.
    pub fn should_generate(
        self,
        seed: i64,
        salt: i32,
        chunk_x: i32,
        chunk_z: i32,
        frequency: f32,
    ) -> bool {
        let mut random = legacy_worldgen_random();
        match self {
            FrequencyReduction::Default => {
                // The game passes (salt, x, z) where (regionX, regionZ, salt)
                // is expected. Kept as is: it is what decides the world.
                random.set_large_feature_with_salt(seed, salt, chunk_x, chunk_z);
                random.next_float() < frequency
            }
            FrequencyReduction::LegacyType1 => {
                let (rx, rz) = (chunk_x >> 4, chunk_z >> 4);
                random.set_seed((rx ^ (rz << 4)) as i64 ^ seed);
                random.next_int();
                random.next_int_bounded((1.0 / frequency) as i32) == 0
            }
            FrequencyReduction::LegacyType2 => {
                random.set_large_feature_with_salt(seed, chunk_x, chunk_z, 10387320);
                random.next_float() < frequency
            }
            FrequencyReduction::LegacyType3 => {
                random.set_large_feature_seed(seed, chunk_x, chunk_z);
                random.next_double() < frequency as f64
            }
        }
    }
}

/// `AbstractSpreadingStructurePlacement.ExclusionZone`: no placement within
/// `chunk_count` chunks of a candidate chunk of another structure set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExclusionZone {
    /// Registry name of the other structure set, e.g. `minecraft:villages`.
    pub other_set: &'static str,
    pub chunk_count: i32,
}

/// `RandomSpreadStructurePlacement`: one attempt per `spacing`×`spacing`
/// chunk region, at a random offset in `[0, spacing - separation)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RandomSpreadPlacement {
    pub locate_offset: (i32, i32, i32),
    pub frequency_reduction: FrequencyReduction,
    pub frequency: f32,
    pub salt: i32,
    pub exclusion_zone: Option<ExclusionZone>,
    pub spacing: i32,
    pub separation: i32,
    pub spread_type: SpreadType,
}

impl RandomSpreadPlacement {
    /// `getPotentialStructureChunk(seed, chunkX, chunkZ)`: the candidate chunk
    /// of the region containing the given chunk.
    pub fn potential_chunk(&self, seed: i64, chunk_x: i32, chunk_z: i32) -> (i32, i32) {
        let region_x = chunk_x.div_euclid(self.spacing);
        let region_z = chunk_z.div_euclid(self.spacing);
        self.region_chunk(seed, region_x, region_z)
    }

    /// The candidate chunk of region `(region_x, region_z)`.
    pub fn region_chunk(&self, seed: i64, region_x: i32, region_z: i32) -> (i32, i32) {
        let mut random = legacy_worldgen_random();
        random.set_large_feature_with_salt(seed, region_x, region_z, self.salt);
        let limit = self.spacing - self.separation;
        let offset_x = self.spread_type.evaluate(&mut random, limit);
        let offset_z = self.spread_type.evaluate(&mut random, limit);
        (
            region_x.wrapping_mul(self.spacing).wrapping_add(offset_x),
            region_z.wrapping_mul(self.spacing).wrapping_add(offset_z),
        )
    }

    /// `isPlacementChunk`.
    pub fn is_placement_chunk(&self, seed: i64, chunk_x: i32, chunk_z: i32) -> bool {
        self.potential_chunk(seed, chunk_x, chunk_z) == (chunk_x, chunk_z)
    }

    /// `applyAdditionalChunkRestrictions`: the frequency roll.
    pub fn apply_additional_chunk_restrictions(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        seed: i64,
    ) -> bool {
        // Java: `frequency < 1f && !shouldGenerate(...)` rejects (NaN passes).
        if self.frequency < 1.0 {
            self.frequency_reduction.should_generate(
                seed,
                self.salt,
                chunk_x,
                chunk_z,
                self.frequency,
            )
        } else {
            true
        }
    }

    /// `StructurePlacement.getLocatePos`: the block `/locate` reports.
    pub fn locate_pos(&self, chunk_x: i32, chunk_z: i32) -> (i32, i32, i32) {
        let (ox, oy, oz) = self.locate_offset;
        (
            chunk_x.wrapping_shl(4).wrapping_add(ox),
            oy,
            chunk_z.wrapping_shl(4).wrapping_add(oz),
        )
    }
}

/// `StructurePlacement` variants used by vanilla structure sets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StructurePlacement {
    RandomSpread(RandomSpreadPlacement),
    /// `ConcentricRingsStructurePlacement` (strongholds). Positions depend on
    /// a biome search; not implemented yet.
    ConcentricRings {
        distance: i32,
        spread: i32,
        count: i32,
        salt: i32,
        preferred_biomes: &'static str,
    },
}

/// `StructureSet`: structures sharing one placement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StructureSet {
    /// Registry name, e.g. `minecraft:villages`.
    pub name: &'static str,
    /// `(structure, weight)`, e.g. `("minecraft:village_plains", 1)`.
    pub structures: &'static [(&'static str, i32)],
    pub placement: StructurePlacement,
}

/// A set of structure sets, so exclusion zones can find the set they refer to.
pub trait StructureSets {
    fn find_set(&self, name: &str) -> Option<&StructureSet>;
}

impl StructureSets for [StructureSet] {
    fn find_set(&self, name: &str) -> Option<&StructureSet> {
        self.iter().find(|s| s.name == name)
    }
}

impl StructureSet {
    /// `StructurePlacement.isStructureChunk`: will this set *attempt* to
    /// generate in this chunk? Returns `None` for placements that are not
    /// supported yet (concentric rings).
    pub fn is_structure_chunk(
        &self,
        sets: &(impl StructureSets + ?Sized),
        seed: i64,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Option<bool> {
        let StructurePlacement::RandomSpread(p) = &self.placement else {
            return None;
        };
        if !p.is_placement_chunk(seed, chunk_x, chunk_z)
            || !p.apply_additional_chunk_restrictions(chunk_x, chunk_z, seed)
        {
            return Some(false);
        }
        if let Some(zone) = p.exclusion_zone {
            let other = sets.find_set(zone.other_set)?;
            // ChunkGeneratorStructureState.hasStructureChunkInRange
            let r = zone.chunk_count;
            for x in chunk_x.wrapping_sub(r)..=chunk_x.wrapping_add(r) {
                for z in chunk_z.wrapping_sub(r)..=chunk_z.wrapping_add(r) {
                    if other.is_structure_chunk(sets, seed, x, z)? {
                        return Some(false);
                    }
                }
            }
        }
        Some(true)
    }
}
