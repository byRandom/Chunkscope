//! End gateways against the real game (`end_gateways.txt`): the block of each
//! of the 20, and the order they open in for many seeds.

use mc_core::structure::end_gateway::{
    GATEWAY_COUNT, RETURN_INDEX, RETURN_STEP, gateway_order, gateway_position, gateways,
    return_gateways,
};
use mc_core::structure::scan::ChunkRect;
use mc_core::world::{Dimension, World};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../../parity/fixtures/game/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("run parity/game-oracle/run.sh")
}

#[test]
fn gateway_positions_and_order_match_the_game() {
    let (mut positions, mut orders) = (0, 0);
    for line in fixture("end_gateways.txt").lines() {
        let mut t = line.split(' ');
        match t.next().unwrap() {
            "position" => {
                let v: Vec<i32> = t.map(|n| n.parse().unwrap()).collect();
                assert_eq!(gateway_position(v[0] as usize), (v[1], v[2]), "{line}");
                positions += 1;
            }
            "order" => {
                let seed: i64 = t.next().unwrap().parse().unwrap();
                let want: Vec<u8> = t.map(|n| n.parse().unwrap()).collect();
                assert_eq!(gateway_order(seed).to_vec(), want, "seed {seed}");
                // The same, with the blocks attached.
                for (n, g) in gateways(seed).iter().enumerate() {
                    assert_eq!((g.x, g.z), gateway_position(want[n] as usize));
                    assert_eq!(g.number as usize, n + 1);
                }
                orders += 1;
            }
            other => panic!("unknown line {other}"),
        }
    }
    assert_eq!(positions, GATEWAY_COUNT);
    assert!(orders >= 400, "{orders}");
}

/// The return gateways of whole regions, as the game's `FeaturePlacer` makes
/// them (`end_return_gateways.txt`): the same set, not just the same points.
#[test]
fn return_gateways_match_the_game() {
    let text = fixture("end_return_gateways.txt");
    let mut lines = text.lines().peekable();
    let mut feature = lines.next().unwrap().split(' ').skip(1);
    assert_eq!(feature.next().unwrap().parse::<i32>().unwrap(), RETURN_STEP);
    assert_eq!(
        feature.next().unwrap().parse::<i32>().unwrap(),
        RETURN_INDEX
    );
    let (mut regions, mut total) = (0, 0);
    while let Some(line) = lines.next() {
        let v: Vec<i64> = line
            .split(' ')
            .skip(1)
            .map(|t| t.parse().unwrap())
            .collect();
        let (seed, rect) = (
            v[0],
            ChunkRect {
                x0: v[1] as i32,
                z0: v[2] as i32,
                x1: v[3] as i32,
                z1: v[4] as i32,
            },
        );
        let mut want = Vec::new();
        while let Some(l) = lines.next_if(|l| l.starts_with("gateway ")) {
            let g: Vec<i32> = l.split(' ').skip(2).map(|t| t.parse().unwrap()).collect();
            // `cx cz x y z`
            want.push((g[0], g[1], (g[2], g[3], g[4])));
        }
        let world = World::new(seed, Dimension::End);
        let got: Vec<(i32, i32, (i32, i32, i32))> = return_gateways(&world, rect)
            .into_iter()
            .map(|g| (g.x >> 4, g.z >> 4, (g.x, g.y, g.z)))
            .collect();
        assert_eq!(got, want, "seed {seed}");
        regions += 1;
        total += want.len();
    }
    assert!(
        regions >= 6 && total > 100,
        "{regions} regions, {total} gateways"
    );
}

/// Return gateways on a real server (`parity/fixtures/server/end_return_gateways.txt`,
/// made by `parity/server-oracle/gateways.sh`): the server has a gateway
/// exactly where Chunkscope puts it, and none one block higher. The last
/// field says whether an end city starts within 48 blocks (`city`), where
/// the city's blocks could raise the surface.
#[test]
fn return_gateways_exist_on_a_real_server() {
    let text = std::fs::read_to_string(format!(
        "{}/../../parity/fixtures/server/end_return_gateways.txt",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("run parity/server-oracle/gateways.sh");
    let (mut present, mut absent, mut missed) = (0, 0, 0);
    for line in text.lines() {
        let t: Vec<&str> = line.split(' ').collect();
        let seed: i64 = t[0].parse().unwrap();
        let (x, y, z): (i32, i32, i32) = (
            t[1].parse().unwrap(),
            t[2].parse().unwrap(),
            t[3].parse().unwrap(),
        );
        let (found, tag) = (t[4] == "yes", t[5]);
        let world = World::new(seed, Dimension::End);
        let chunk = ChunkRect {
            x0: x >> 4,
            z0: z >> 4,
            x1: x >> 4,
            z1: z >> 4,
        };
        let ours = return_gateways(&world, chunk);
        if tag == "above" {
            // One block above the predicted one.
            assert!(!found, "{line}");
            assert!(
                ours.iter().any(|g| (g.x, g.y + 1, g.z) == (x, y, z)),
                "{line}"
            );
            absent += 1;
        } else {
            assert_eq!(ours.len(), 1, "{line}");
            assert_eq!((ours[0].x, ours[0].y, ours[0].z), (x, y, z), "{line}");
            // Where the terrain height is right the server has the gateway at
            // that block. Where an end city's piece covers the column, its
            // blocks may have raised the surface: Chunkscope says so.
            assert!(
                found || ours[0].under_city,
                "the server has no gateway at {line}, and no city covers it"
            );
            if !found {
                missed += 1;
            }
            // The tag was written from the same answer: it pins it.
            assert_eq!(ours[0].under_city, tag == "covered", "{line}");
            present += 1;
        }
    }
    // Most predictions are exact; the misses are the gateways under a city.
    assert!(
        present - missed >= 20 && missed >= 5,
        "{present} present, {missed} missed"
    );
    assert!(
        present >= 10 && absent >= 10,
        "{present} present, {absent} absent"
    );
}
