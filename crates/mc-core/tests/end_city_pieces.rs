//! End city pieces against the real game: `EndCityPieces.startHouseTower`
//! for many chunks (`end_city_pieces.txt`), and the pieces of cities that
//! really generate (`end_city_structures.txt`). Pieces are compared whole:
//! template, rotation, gen depth and bounding box, in the game's order.

use mc_core::structure::end_city::{CityPiece, end_city_has_ship, end_city_pieces};
use mc_core::structure::validate::Structure;
use mc_core::world::{Dimension, World};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../../parity/fixtures/game/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("run parity/game-oracle/run.sh")
}

fn format(p: &CityPiece) -> String {
    let b = p.bbox;
    format!(
        "{}/{}/{}/{},{},{},{},{},{}",
        p.name, p.rotation, p.gen_depth, b[0], b[1], b[2], b[3], b[4], b[5]
    )
}

#[test]
fn end_city_pieces_match_the_game() {
    let (mut cities, mut ships, mut pieces) = (0, 0, 0);
    for line in fixture("end_city_pieces.txt").lines() {
        let mut parts = line.split(' ');
        let seed: i64 = parts.next().unwrap().parse().unwrap();
        let cx: i32 = parts.next().unwrap().parse().unwrap();
        let cz: i32 = parts.next().unwrap().parse().unwrap();
        let y: i32 = parts.next().unwrap().parse().unwrap();
        let expected: Vec<&str> = parts.collect();
        let got = end_city_pieces(seed, cx, cz, [cx * 16 + 7, y, cz * 16 + 7]);
        let got: Vec<String> = got.iter().map(format).collect();
        assert_eq!(got, expected, "seed {seed}, chunk {cx}, {cz}");
        let ship = expected.iter().any(|p| p.starts_with("ship/"));
        assert_eq!(
            end_city_has_ship(seed, cx, cz),
            ship,
            "seed {seed}, chunk {cx}, {cz}"
        );
        cities += 1;
        ships += usize::from(ship);
        pieces += expected.len();
    }
    println!("{cities} cities, {pieces} pieces, {ships} with a ship");
    assert!(cities == 600 && ships > 100 && ships < 500);
}

#[test]
fn generated_end_cities_match_the_game() {
    let city = Structure::find("minecraft:end_city").unwrap();
    let mut checked = 0;
    for line in fixture("end_city_structures.txt").lines() {
        let mut parts = line.split(' ');
        let seed: i64 = parts.next().unwrap().parse().unwrap();
        let cx: i32 = parts.next().unwrap().parse().unwrap();
        let cz: i32 = parts.next().unwrap().parse().unwrap();
        let start: Vec<i32> = parts
            .next()
            .unwrap()
            .split(',')
            .map(|v| v.parse().unwrap())
            .collect();
        let expected: Vec<&str> = parts.collect();
        let world = World::new(seed, Dimension::End);
        let point = world
            .structures()
            .try_structure(city, cx, cz)
            .expect("the city generates");
        assert_eq!([point.0, point.1, point.2], start[..]);
        let got: Vec<String> = end_city_pieces(seed, cx, cz, [point.0, point.1, point.2])
            .iter()
            .map(format)
            .collect();
        assert_eq!(got, expected, "seed {seed}, chunk {cx}, {cz}");
        checked += 1;
    }
    assert!(checked >= 30);
}
