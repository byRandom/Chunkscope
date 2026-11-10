//! End city pieces (`EndCityPieces`, 26.x): the towers, bridges, houses and
//! the ship an end city is built from, generated like the game does from the
//! city's start. The pieces do not depend on the terrain, only on the world
//! seed, the chunk and the start position.

use crate::random::{LegacyRandomSource, RandomSource, WorldgenRandom};

use super::generated::END_CITY_TEMPLATES;

/// `Rotation`, by ordinal: `NONE`, `CLOCKWISE_90`, `CLOCKWISE_180`,
/// `COUNTERCLOCKWISE_90`.
pub type Rotation = u8;

/// One placed piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CityPiece {
    /// The template, `end_city/<name>`.
    pub name: &'static str,
    pub rotation: Rotation,
    /// `StructurePiece.genDepth`, as the game leaves it.
    pub gen_depth: i32,
    /// `[min_x, min_y, min_z, max_x, max_y, max_z]`
    pub bbox: [i32; 6],
    /// The template's origin (`templatePosition`).
    position: [i32; 3],
}

impl CityPiece {
    /// `new EndCityPiece(manager, name, position, rotation, overwrite)`: the
    /// template's bounding box at `position` (no mirror, pivot at the origin).
    fn new(name: &'static str, position: [i32; 3], rotation: Rotation) -> Self {
        let [sx, sy, sz] = template_size(name);
        let a = transform([0, 0, 0], rotation);
        let b = transform([sx - 1, sy - 1, sz - 1], rotation);
        let bbox = [
            a[0].min(b[0]) + position[0],
            a[1].min(b[1]) + position[1],
            a[2].min(b[2]) + position[2],
            a[0].max(b[0]) + position[0],
            a[1].max(b[1]) + position[1],
            a[2].max(b[2]) + position[2],
        ];
        Self {
            name,
            rotation,
            gen_depth: 0,
            bbox,
            position,
        }
    }

    /// `EndCityPieces.addPiece`: a piece at this one's origin, moved by
    /// `offset` turned by this piece's rotation.
    fn add(&self, offset: [i32; 3], name: &'static str, rotation: Rotation) -> Self {
        let d = transform(offset, self.rotation);
        let p = self.position;
        Self::new(name, [p[0] + d[0], p[1] + d[1], p[2] + d[2]], rotation)
    }

    /// `BoundingBox.intersects`: inclusive on every axis.
    fn intersects(&self, other: &CityPiece) -> bool {
        let (a, b) = (self.bbox, other.bbox);
        a[3] >= b[0] && a[0] <= b[3] && a[5] >= b[2] && a[2] <= b[5] && a[4] >= b[1] && a[1] <= b[4]
    }
}

fn template_size(name: &str) -> [i32; 3] {
    END_CITY_TEMPLATES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, size)| *size)
        .expect("end city template")
}

/// `StructureTemplate.transform` with no mirror and the pivot at the origin.
fn transform([x, y, z]: [i32; 3], rotation: Rotation) -> [i32; 3] {
    match rotation {
        1 => [-z, y, x],  // CLOCKWISE_90
        2 => [-x, y, -z], // CLOCKWISE_180
        3 => [z, y, -x],  // COUNTERCLOCKWISE_90
        _ => [x, y, z],
    }
}

/// `Rotation.getRotated`.
fn rotated(a: Rotation, b: Rotation) -> Rotation {
    (a + b) % 4
}

/// `EndCityPieces.TOWER_BRIDGES`
const TOWER_BRIDGES: [(Rotation, [i32; 3]); 4] = [
    (0, [1, -1, 0]),
    (1, [6, -1, 1]),
    (3, [0, -1, 5]),
    (2, [5, -1, 6]),
];

/// `EndCityPieces.FAT_TOWER_BRIDGES`
const FAT_TOWER_BRIDGES: [(Rotation, [i32; 3]); 4] = [
    (0, [4, -1, 0]),
    (1, [12, -1, 4]),
    (3, [0, -1, 8]),
    (2, [8, -1, 12]),
];

/// `EndCityPieces.SectionGenerator`s.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Section {
    HouseTower,
    Tower,
    TowerBridge,
    FatTower,
}

struct Builder<R: RandomSource> {
    random: R,
    /// `TOWER_BRIDGE_GENERATOR.shipCreated`: one ship per city.
    ship_created: bool,
}

impl<R: RandomSource> Builder<R> {
    /// `EndCityPieces.recursiveChildren`: the section's pieces are kept only
    /// if none of them hits a piece already in `pieces` from another branch.
    fn recursive_children(
        &mut self,
        section: Section,
        depth: i32,
        parent: CityPiece,
        offset: Option<[i32; 3]>,
        pieces: &mut Vec<CityPiece>,
    ) -> bool {
        if depth > 8 {
            return false;
        }
        let mut children = Vec::new();
        if !self.generate(section, depth, parent, offset, &mut children) {
            return false;
        }
        let gen_depth = self.random.next_int();
        for child in &mut children {
            child.gen_depth = gen_depth;
            if let Some(hit) = pieces.iter().find(|p| p.intersects(child))
                && hit.gen_depth != parent.gen_depth
            {
                return false;
            }
        }
        pieces.extend(children);
        true
    }

    /// `SectionGenerator.generate`
    fn generate(
        &mut self,
        section: Section,
        depth: i32,
        parent: CityPiece,
        offset: Option<[i32; 3]>,
        pieces: &mut Vec<CityPiece>,
    ) -> bool {
        let rot = parent.rotation;
        let add = |pieces: &mut Vec<CityPiece>, from: &CityPiece, at, name, r| {
            let piece = from.add(at, name, r);
            pieces.push(piece);
            piece
        };
        match section {
            Section::HouseTower => {
                if depth > 8 {
                    return false;
                }
                let at = offset.expect("house towers start at an offset");
                let mut p = add(pieces, &parent, at, "base_floor", rot);
                match self.random.next_int_bounded(3) {
                    0 => {
                        add(pieces, &p, [-1, 4, -1], "base_roof", rot);
                    }
                    1 => {
                        p = add(pieces, &p, [-1, 0, -1], "second_floor_2", rot);
                        p = add(pieces, &p, [-1, 8, -1], "second_roof", rot);
                        self.recursive_children(Section::Tower, depth + 1, p, None, pieces);
                    }
                    2 => {
                        p = add(pieces, &p, [-1, 0, -1], "second_floor_2", rot);
                        p = add(pieces, &p, [-1, 4, -1], "third_floor_2", rot);
                        p = add(pieces, &p, [-1, 8, -1], "third_roof", rot);
                        self.recursive_children(Section::Tower, depth + 1, p, None, pieces);
                    }
                    _ => unreachable!(),
                }
                true
            }
            Section::Tower => {
                let x = 3 + self.random.next_int_bounded(2);
                let z = 3 + self.random.next_int_bounded(2);
                let mut p = add(pieces, &parent, [x, -3, z], "tower_base", rot);
                p = add(pieces, &p, [0, 7, 0], "tower_piece", rot);
                let mut bridge_from = (self.random.next_int_bounded(3) == 0).then_some(p);
                let floors = 1 + self.random.next_int_bounded(3);
                for i in 0..floors {
                    p = add(pieces, &p, [0, 4, 0], "tower_piece", rot);
                    if i < floors - 1 && self.random.next_boolean() {
                        bridge_from = Some(p);
                    }
                }
                if let Some(from) = bridge_from {
                    for (r, at) in TOWER_BRIDGES {
                        if self.random.next_boolean() {
                            let bridge = add(pieces, &from, at, "bridge_end", rotated(rot, r));
                            self.recursive_children(
                                Section::TowerBridge,
                                depth + 1,
                                bridge,
                                None,
                                pieces,
                            );
                        }
                    }
                    add(pieces, &p, [-1, 4, -1], "tower_top", rot);
                } else if depth == 7 {
                    add(pieces, &p, [-1, 4, -1], "tower_top", rot);
                } else {
                    return self.recursive_children(Section::FatTower, depth + 1, p, None, pieces);
                }
                true
            }
            Section::TowerBridge => {
                let length = self.random.next_int_bounded(4) + 1;
                let mut p = add(pieces, &parent, [0, 0, -4], "bridge_piece", rot);
                pieces.last_mut().expect("just added").gen_depth = -1;
                p.gen_depth = -1;
                let mut y = 0;
                for _ in 0..length {
                    if self.random.next_boolean() {
                        p = add(pieces, &p, [0, y, -4], "bridge_piece", rot);
                        y = 0;
                    } else {
                        p = if self.random.next_boolean() {
                            add(pieces, &p, [0, y, -4], "bridge_steep_stairs", rot)
                        } else {
                            add(pieces, &p, [0, y, -8], "bridge_gentle_stairs", rot)
                        };
                        y = 4;
                    }
                }
                if !self.ship_created && self.random.next_int_bounded(10 - depth) == 0 {
                    let x = -8 + self.random.next_int_bounded(8);
                    let z = -70 + self.random.next_int_bounded(10);
                    add(pieces, &p, [x, y, z], "ship", rot);
                    self.ship_created = true;
                } else if !self.recursive_children(
                    Section::HouseTower,
                    depth + 1,
                    p,
                    Some([-3, y + 1, -11]),
                    pieces,
                ) {
                    return false;
                }
                add(pieces, &p, [4, y, 0], "bridge_end", rotated(rot, 2));
                pieces.last_mut().expect("just added").gen_depth = -1;
                true
            }
            Section::FatTower => {
                let mut p = add(pieces, &parent, [-3, 4, -3], "fat_tower_base", rot);
                p = add(pieces, &p, [0, 4, 0], "fat_tower_middle", rot);
                for _ in 0..2 {
                    if self.random.next_int_bounded(3) == 0 {
                        break;
                    }
                    p = add(pieces, &p, [0, 8, 0], "fat_tower_middle", rot);
                    for (r, at) in FAT_TOWER_BRIDGES {
                        if self.random.next_boolean() {
                            let bridge = add(pieces, &p, at, "bridge_end", rotated(rot, r));
                            self.recursive_children(
                                Section::TowerBridge,
                                depth + 1,
                                bridge,
                                None,
                                pieces,
                            );
                        }
                    }
                }
                add(pieces, &p, [-2, 8, -2], "fat_tower_top", rot);
                true
            }
        }
    }
}

/// `EndCityPieces.startHouseTower` with `random` as the structure's
/// generation random left it.
fn start_house_tower(
    start: [i32; 3],
    rotation: Rotation,
    random: impl RandomSource,
) -> Vec<CityPiece> {
    let mut builder = Builder {
        random,
        ship_created: false,
    };
    let mut pieces = Vec::new();
    let mut p = CityPiece::new("base_floor", start, rotation);
    pieces.push(p);
    for (at, name) in [
        ([-1, 0, -1], "second_floor_1"),
        ([-1, 4, -1], "third_floor_1"),
        ([-1, 8, -1], "third_roof"),
    ] {
        p = p.add(at, name, rotation);
        pieces.push(p);
    }
    builder.recursive_children(Section::Tower, 1, p, None, &mut pieces);
    pieces
}

/// The pieces of the end city that starts at `start` in chunk
/// (`chunk_x`, `chunk_z`): the chunk's large feature seed, the rotation
/// draw of `EndCityStructure.findGenerationPoint`, then the pieces.
/// `start` must be the city's validated generation point.
pub fn end_city_pieces(seed: i64, chunk_x: i32, chunk_z: i32, start: [i32; 3]) -> Vec<CityPiece> {
    let mut random = WorldgenRandom::new(LegacyRandomSource::new(0));
    random.set_large_feature_seed(seed, chunk_x, chunk_z);
    let rotation = random.next_int_bounded(4) as Rotation;
    start_house_tower(start, rotation, random)
}

/// Whether the end city in chunk (`chunk_x`, `chunk_z`) has a ship. The
/// pieces' relative layout, and so the ship, does not depend on the start.
pub fn end_city_has_ship(seed: i64, chunk_x: i32, chunk_z: i32) -> bool {
    end_city_pieces(seed, chunk_x, chunk_z, [0, 0, 0])
        .iter()
        .any(|p| p.name == "ship")
}
