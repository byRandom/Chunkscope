//! End gateways: the 20 that appear around the main island, one after each
//! dragon defeat (`EnderDragonFight`).
//!
//! Their positions are the same in every world: points on a circle of radius
//! 96 around (0, 0), at height 75. The seed decides only the order they
//! appear in: a shuffle of 0 to 19 with the game's single-threaded random
//! source (the `java.util.Random` generator) seeded with the world seed.

use crate::random::{LegacyRandomSource, RandomSource};

/// How many gateways the game places.
pub const GATEWAY_COUNT: usize = 20;
/// Height of every gateway (the block of the gateway itself).
pub const GATEWAY_Y: i32 = 75;

/// The block `(x, z)` of gateway `index` (0 to 19):
/// `floor(96 * cos(a))`, `floor(96 * sin(a))` with
/// `a = 2 * (-π + (π / 20) * index)`.
///
/// A table, not the formula: `x` and `z` of some gateways are a hair above
/// or below a whole number (`cos` of a right angle is `±6e-17`), so the sign
/// of the last bit decides the block, and a platform's `cos` must not. A test
/// checks the table against the formula and the game's fixture.
pub fn gateway_position(index: usize) -> (i32, i32) {
    POSITIONS[index]
}

const POSITIONS: [(i32, i32); GATEWAY_COUNT] = [
    (96, 0),
    (91, 29),
    (77, 56),
    (56, 77),
    (29, 91),
    (-1, 96),
    (-30, 91),
    (-57, 77),
    (-78, 56),
    (-92, 29),
    (-96, -1),
    (-92, -30),
    (-78, -57),
    (-57, -78),
    (-30, -92),
    (0, -96),
    (29, -92),
    (56, -78),
    (77, -57),
    (91, -30),
];

/// The order the gateways appear in: element `n` is the index (into
/// [`gateway_position`]) of the gateway the `n + 1`th dragon defeat opens.
/// `Util.shuffle` of `0..20` with `RandomSource.createThreadLocalInstance(seed)`.
pub fn gateway_order(seed: i64) -> [u8; GATEWAY_COUNT] {
    let mut order: [u8; GATEWAY_COUNT] = std::array::from_fn(|i| i as u8);
    let mut random = LegacyRandomSource::new(seed);
    // Util.shuffle: for i from the size down to 2, swap element i - 1 with
    // a random one among the first i.
    for i in (2..=GATEWAY_COUNT).rev() {
        let j = random.next_int_bounded(i as i32) as usize;
        order.swap(i - 1, j);
    }
    order
}

/// A gateway of a world: its block, and which dragon defeat opens it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gateway {
    pub x: i32,
    pub z: i32,
    /// 1 for the first gateway (after the first dragon defeat), up to 20.
    pub number: u8,
}

/// The 20 gateways of a world, in the order they appear.
pub fn gateways(seed: i64) -> Vec<Gateway> {
    gateway_order(seed)
        .iter()
        .enumerate()
        .map(|(n, &i)| {
            let (x, z) = gateway_position(i as usize);
            Gateway {
                x,
                z,
                number: n as u8 + 1,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The game's expression, evaluated here (see `gateway_position`).
    fn formula(index: usize) -> (i32, i32) {
        let a = 2.0 * (-std::f64::consts::PI + 0.15707963267948966 * index as f64);
        (
            (96.0 * a.cos()).floor() as i32,
            (96.0 * a.sin()).floor() as i32,
        )
    }

    #[test]
    fn table_is_the_formula() {
        for i in 0..GATEWAY_COUNT {
            assert_eq!(gateway_position(i), formula(i), "gateway {i}");
        }
    }

    #[test]
    fn every_gateway_appears_once() {
        for seed in [0, 1, -1, 123456789, i64::MIN, i64::MAX] {
            let mut order = gateway_order(seed).to_vec();
            order.sort();
            assert_eq!(order, (0..20).collect::<Vec<u8>>(), "seed {seed}");
            let numbers: Vec<u8> = gateways(seed).iter().map(|g| g.number).collect();
            assert_eq!(numbers, (1..=20).collect::<Vec<u8>>());
        }
    }
}
