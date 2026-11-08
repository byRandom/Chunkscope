//! Volume-mode (terrain) parity against the real game
//! (parity/game-oracle/GameOracle.java).

use mc_core::density::DensityVolume;
use mc_core::noise::{NormalNoise, vanilla};

const VOLUME_NOISE: &str = include_str!("../../../parity/fixtures/game/volume_noise.txt");

fn fnv(values: &[f32]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for v in values {
        h ^= v.to_bits() as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn f64_bits(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).unwrap())
}

#[test]
fn noise_add_to_volume_matches_game() {
    let mut shapes = Vec::new();
    let mut checked = 0;
    for line in VOLUME_NOISE.lines() {
        let t: Vec<&str> = line.split(' ').collect();
        if t[0] == "shape" {
            let v: Vec<i32> = t[2..11].iter().map(|x| x.parse().unwrap()).collect();
            let volume =
                DensityVolume::with_steps(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8]);
            shapes.push((t[1].to_string(), volume, f64_bits(t[11]), f64_bits(t[12])));
            continue;
        }
        let seed: i64 = t[0].parse().unwrap();
        let name = t[1];
        let (_, params) = vanilla::ALL.iter().find(|(n, _)| *n == name).expect(name);
        let noise = NormalNoise::instantiate(seed, name, params);
        for ((shape, volume, xz, y), want) in shapes.iter().zip(&t[2..]) {
            let mut buffer = vec![0f32; volume.size()];
            noise.add_to_volume(&mut buffer, volume, *xz, *y, 1.0);
            noise.add_to_volume(&mut buffer, volume, *xz, *y, -0.75);
            let n = buffer.len();
            let got = format!(
                "{shape}={:x},{:x},{:x},{:x}",
                fnv(&buffer),
                buffer[0].to_bits(),
                buffer[n / 2].to_bits(),
                buffer[n - 1].to_bits()
            );
            assert_eq!(&got, want, "{name} seed {seed}");
            checked += 1;
        }
    }
    assert_eq!(checked, 3 * vanilla::ALL.len() * shapes.len());
}

const FINAL_DENSITY: &str = include_str!("../../../parity/fixtures/game/final_density.txt");

/// Every node of the compiled overworld `final_density`, over full block
/// columns, compared with the game's own sampler for that node.
#[test]
fn final_density_nodes_match_game() {
    use mc_core::density::{DensitySampler, generated::OVERWORLD_FINAL_DENSITY};
    let tree = OVERWORLD_FINAL_DENSITY;
    let mut failures = Vec::new();
    let mut sampler: Option<(i64, DensitySampler)> = None;
    for line in FINAL_DENSITY.lines() {
        let t: Vec<&str> = line.split(' ').collect();
        let seed: i64 = t[0].parse().unwrap();
        let (x, z): (i32, i32) = (t[1].parse().unwrap(), t[2].parse().unwrap());
        if sampler.as_ref().is_none_or(|(s, _)| *s != seed) {
            sampler = Some((seed, DensitySampler::new(tree, seed)));
        }
        let s = &sampler.as_ref().unwrap().1;
        let volume = DensityVolume::column(x, -64, 384, z);
        assert_eq!(t.len() - 3, tree.nodes.len());
        for (id, want) in t[3..].iter().enumerate() {
            let mut buffer = vec![0f32; volume.size()];
            s.sample_node(id as u32, &mut buffer, &volume);
            let n = buffer.len();
            let got = format!(
                "{:x},{:x},{:x},{:x}",
                fnv(&buffer),
                buffer[0].to_bits(),
                buffer[n / 2].to_bits(),
                buffer[n - 1].to_bits()
            );
            if &got != want {
                failures.push(format!(
                    "seed {seed} column ({x}, {z}) node {id} {:?}: rust {got} game {want}",
                    tree.nodes[id]
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} node mismatches, first ones:\n{}",
        failures.len(),
        failures
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
