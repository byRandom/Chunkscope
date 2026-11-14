//! Mineshaft pieces (`MineshaftPieces`, 26.x): the room, corridors,
//! crossings and stairs a mineshaft is built from. Their bounding boxes and
//! the random numbers they use decide how far the game moves the mineshaft
//! down (or, in badlands, up), and so the height where its biome is checked.

use crate::random::RandomSource;

/// `MineshaftStructure.Type`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MineshaftType {
    Normal,
    /// Badlands mineshafts, raised towards the surface.
    Mesa,
}

/// `Direction`, horizontal only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    North,
    South,
    West,
    East,
}

/// The kind of a mineshaft piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PieceKind {
    Room,
    Corridor,
    /// A crossing, two floors high or not.
    Crossing {
        two_floored: bool,
    },
    Stairs,
}

/// One placed piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MineshaftPiece {
    pub kind: PieceKind,
    /// `[min_x, min_y, min_z, max_x, max_y, max_z]`
    pub bbox: [i32; 6],
    /// `StructurePiece.genDepth`
    pub gen_depth: i32,
    /// The piece's orientation (the room has none).
    pub direction: Option<Direction>,
}

/// `new BoundingBox(...)`: the game logs and swaps reversed corners; the
/// mineshaft never builds one.
fn bbox(x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32) -> [i32; 6] {
    [
        x0.min(x1),
        y0.min(y1),
        z0.min(z1),
        x0.max(x1),
        y0.max(y1),
        z0.max(z1),
    ]
}

fn moved(b: [i32; 6], x: i32, y: i32, z: i32) -> [i32; 6] {
    [b[0] + x, b[1] + y, b[2] + z, b[3] + x, b[4] + y, b[5] + z]
}

/// `BoundingBox.intersects`: inclusive on every axis.
fn intersects(a: &[i32; 6], b: &[i32; 6]) -> bool {
    a[3] >= b[0] && a[0] <= b[3] && a[5] >= b[2] && a[2] <= b[5] && a[4] >= b[1] && a[1] <= b[4]
}

/// `BoundingBox.encapsulatingBoxes` of the pieces.
pub fn encapsulating(pieces: &[MineshaftPiece]) -> [i32; 6] {
    pieces.iter().skip(1).fold(pieces[0].bbox, |a, p| {
        let b = p.bbox;
        [
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[2].min(b[2]),
            a[3].max(b[3]),
            a[4].max(b[4]),
            a[5].max(b[5]),
        ]
    })
}

struct Builder<'r, R> {
    random: &'r mut R,
    pieces: Vec<MineshaftPiece>,
}

impl<R: RandomSource> Builder<'_, R> {
    /// `StructurePiecesBuilder.findCollisionPiece`
    fn collides(&self, b: &[i32; 6]) -> bool {
        self.pieces.iter().any(|p| intersects(&p.bbox, b))
    }

    /// `MineshaftPieces.generateAndAddPiece`: `depth` is the parent's.
    fn generate_and_add(&mut self, x: i32, y: i32, z: i32, dir: Direction, depth: i32) {
        if depth > 8 {
            return;
        }
        let start = self.pieces[0].bbox;
        if (x - start[0]).abs() > 80 || (z - start[2]).abs() > 80 {
            return;
        }
        if let Some(piece) = self.random_shaft_piece(x, y, z, dir, depth + 1) {
            self.pieces.push(piece);
            self.add_children(self.pieces.len() - 1);
        }
    }

    /// `MineshaftPieces.createRandomShaftPiece`
    fn random_shaft_piece(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        dir: Direction,
        gen_depth: i32,
    ) -> Option<MineshaftPiece> {
        let r = self.random.next_int_bounded(100);
        let (kind, bbox) = if r >= 80 {
            let b = self.find_crossing(x, y, z, dir)?;
            // `isTwoFloored`: taller than 3 blocks.
            let two_floored = b[4] - b[1] + 1 > 3;
            (PieceKind::Crossing { two_floored }, b)
        } else if r >= 70 {
            (PieceKind::Stairs, self.find_stairs(x, y, z, dir)?)
        } else {
            let b = self.find_corridor_size(x, y, z, dir)?;
            // The corridor's rails and spider webs, decided in its constructor.
            let has_rails = self.random.next_int_bounded(3) == 0;
            if !has_rails {
                self.random.next_int_bounded(23);
            }
            (PieceKind::Corridor, b)
        };
        Some(MineshaftPiece {
            kind,
            bbox,
            gen_depth,
            direction: Some(dir),
        })
    }

    /// `MineShaftCorridor.findCorridorSize`
    fn find_corridor_size(&mut self, x: i32, y: i32, z: i32, dir: Direction) -> Option<[i32; 6]> {
        let mut n = self.random.next_int_bounded(3) + 2;
        while n > 0 {
            let len = n * 5;
            let b = match dir {
                Direction::North => bbox(0, 0, -(len - 1), 2, 2, 0),
                Direction::South => bbox(0, 0, 0, 2, 2, len - 1),
                Direction::West => bbox(-(len - 1), 0, 0, 0, 2, 2),
                Direction::East => bbox(0, 0, 0, len - 1, 2, 2),
            };
            let b = moved(b, x, y, z);
            if !self.collides(&b) {
                return Some(b);
            }
            n -= 1;
        }
        None
    }

    /// `MineShaftCrossing.findCrossing`
    fn find_crossing(&mut self, x: i32, y: i32, z: i32, dir: Direction) -> Option<[i32; 6]> {
        let h = if self.random.next_int_bounded(4) == 0 {
            6
        } else {
            2
        };
        let b = match dir {
            Direction::North => bbox(-1, 0, -4, 3, h, 0),
            Direction::South => bbox(-1, 0, 0, 3, h, 4),
            Direction::West => bbox(-4, 0, -1, 0, h, 3),
            Direction::East => bbox(0, 0, -1, 4, h, 3),
        };
        let b = moved(b, x, y, z);
        (!self.collides(&b)).then_some(b)
    }

    /// `MineShaftStairs.findStairs`
    fn find_stairs(&mut self, x: i32, y: i32, z: i32, dir: Direction) -> Option<[i32; 6]> {
        let b = match dir {
            Direction::North => bbox(0, -5, -8, 2, 2, 0),
            Direction::South => bbox(0, -5, 0, 2, 2, 8),
            Direction::West => bbox(-8, -5, 0, 0, 2, 2),
            Direction::East => bbox(0, -5, 0, 8, 2, 2),
        };
        let b = moved(b, x, y, z);
        (!self.collides(&b)).then_some(b)
    }

    /// `addChildren` of the piece at `index`.
    fn add_children(&mut self, index: usize) {
        let MineshaftPiece {
            kind,
            bbox: [min_x, min_y, min_z, max_x, max_y, max_z],
            gen_depth: depth,
            direction,
        } = self.pieces[index];
        use Direction::*;
        match kind {
            PieceKind::Room => {
                let x_span = max_x - min_x + 1;
                let z_span = max_z - min_z + 1;
                let y_rand = (max_y - min_y + 1 - 3 - 1).max(1);
                // North, south, west, then east walls.
                for side in [North, South, West, East] {
                    let span = if matches!(side, North | South) {
                        x_span
                    } else {
                        z_span
                    };
                    let mut i = 0;
                    while i < span {
                        i += self.random.next_int_bounded(span);
                        if i + 3 > span {
                            break;
                        }
                        let y = min_y + self.random.next_int_bounded(y_rand) + 1;
                        let (x, z) = match side {
                            North => (min_x + i, min_z - 1),
                            South => (min_x + i, max_z + 1),
                            West => (min_x - 1, min_z + i),
                            East => (max_x + 1, min_z + i),
                        };
                        self.generate_and_add(x, y, z, side, depth);
                        i += 4;
                    }
                }
            }
            PieceKind::Corridor => {
                let Some(dir) = direction else { return };
                let r = self.random.next_int_bounded(4);
                let y = min_y - 1 + self.random.next_int_bounded(3);
                let (x, z, d) = match (dir, r) {
                    (North, 0 | 1) => (min_x, min_z - 1, North),
                    (North, 2) => (min_x - 1, min_z, West),
                    (North, _) => (max_x + 1, min_z, East),
                    (South, 0 | 1) => (min_x, max_z + 1, South),
                    (South, 2) => (min_x - 1, max_z - 3, West),
                    (South, _) => (max_x + 1, max_z - 3, East),
                    (West, 0 | 1) => (min_x - 1, min_z, West),
                    (West, 2) => (min_x, min_z - 1, North),
                    (West, _) => (min_x, max_z + 1, South),
                    (East, 0 | 1) => (max_x + 1, min_z, East),
                    (East, 2) => (max_x - 3, min_z - 1, North),
                    (East, _) => (max_x - 3, max_z + 1, South),
                };
                self.generate_and_add(x, y, z, d, depth);
                if depth < 8 {
                    if matches!(dir, North | South) {
                        let mut z = min_z + 3;
                        while z + 3 <= max_z {
                            match self.random.next_int_bounded(5) {
                                0 => self.generate_and_add(min_x - 1, min_y, z, West, depth + 1),
                                1 => self.generate_and_add(max_x + 1, min_y, z, East, depth + 1),
                                _ => {}
                            }
                            z += 5;
                        }
                    } else {
                        let mut x = min_x + 3;
                        while x + 3 <= max_x {
                            match self.random.next_int_bounded(5) {
                                0 => self.generate_and_add(x, min_y, min_z - 1, North, depth + 1),
                                1 => self.generate_and_add(x, min_y, max_z + 1, South, depth + 1),
                                _ => {}
                            }
                            x += 5;
                        }
                    }
                }
            }
            PieceKind::Crossing { two_floored } => {
                let Some(dir) = direction else { return };
                let north = (min_x + 1, min_y, min_z - 1, North);
                let south = (min_x + 1, min_y, max_z + 1, South);
                let west = (min_x - 1, min_y, min_z + 1, West);
                let east = (max_x + 1, min_y, min_z + 1, East);
                let exits = match dir {
                    North => [north, west, east],
                    South => [south, west, east],
                    West => [north, south, west],
                    East => [north, south, east],
                };
                for (x, y, z, d) in exits {
                    self.generate_and_add(x, y, z, d, depth);
                }
                if two_floored {
                    for (x, y, z, d) in [north, west, east, south] {
                        if self.random.next_boolean() {
                            self.generate_and_add(x, y + 4, z, d, depth);
                        }
                    }
                }
            }
            PieceKind::Stairs => {
                let Some(dir) = direction else { return };
                let (x, z) = match dir {
                    North => (min_x, min_z - 1),
                    South => (min_x, max_z + 1),
                    West => (min_x - 1, min_z),
                    East => (max_x + 1, min_z),
                };
                self.generate_and_add(x, min_y, z, dir, depth);
            }
        }
    }
}

/// `MineshaftStructure.generatePiecesAndAdjust` up to the vertical move: the
/// room at the chunk's (2, 2) block and every piece that grows from it, in
/// the game's order. `random` is the structure's, after its first
/// `nextDouble`.
pub fn mineshaft_pieces(
    random: &mut impl RandomSource,
    chunk_x: i32,
    chunk_z: i32,
) -> Vec<MineshaftPiece> {
    let (x, z) = ((chunk_x << 4) + 2, (chunk_z << 4) + 2);
    // `new MineShaftRoom(0, random, x, z, type)`
    let x1 = x + 7 + random.next_int_bounded(6);
    let y1 = 54 + random.next_int_bounded(6);
    let z1 = z + 7 + random.next_int_bounded(6);
    let mut builder = Builder {
        random,
        pieces: vec![MineshaftPiece {
            kind: PieceKind::Room,
            bbox: bbox(x, 50, z, x1, y1, z1),
            gen_depth: 0,
            direction: None,
        }],
    };
    builder.add_children(0);
    builder.pieces
}
