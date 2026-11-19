//! End gateways against the real game (`end_gateways.txt`): the block of each
//! of the 20, and the order they open in for many seeds.

use mc_core::structure::end_gateway::{GATEWAY_COUNT, gateway_order, gateway_position, gateways};

fn fixture() -> String {
    std::fs::read_to_string(format!(
        "{}/../../parity/fixtures/game/end_gateways.txt",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("run parity/game-oracle/run.sh")
}

#[test]
fn gateway_positions_and_order_match_the_game() {
    let (mut positions, mut orders) = (0, 0);
    for line in fixture().lines() {
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
