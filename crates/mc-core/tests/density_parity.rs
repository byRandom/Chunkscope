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

fn volume_hash(
    s: &mc_core::density::DensitySampler,
    ctx: &mc_core::density::EvalContext,
    id: u32,
    volume: &DensityVolume,
) -> String {
    let mut buffer = vec![0f32; volume.size()];
    s.sample_node(ctx, id, &mut buffer, volume);
    let n = buffer.len();
    format!(
        "{:x},{:x},{:x},{:x}",
        fnv(&buffer),
        buffer[0].to_bits(),
        buffer[n / 2].to_bits(),
        buffer[n - 1].to_bits()
    )
}

/// Every node of the compiled overworld density trees (`final_density` and
/// the aquifer's functions), compared with the game's own sampler for that
/// node, each in a fresh caching context and in the oracle's order: a 3x1x3
/// layer (volume mode), 4 points (point mode, which can read cached volume
/// values), then a full block column (volume mode).
#[test]
fn density_nodes_match_game() {
    use mc_core::density::{DensitySampler, EvalContext, generated::OVERWORLD_FINAL_DENSITY};
    let tree = OVERWORLD_FINAL_DENSITY;
    let ys = [-64, -10, 63, 140];
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
        let column = DensityVolume::column(x, -64, 384, z);
        let layer = DensityVolume::with_steps(3, 1, 3, x - 4, 0, z - 4, 4, 1, 4);
        assert_eq!(t.len() - 3, tree.nodes.len());
        for (id, want) in t[3..].iter().enumerate() {
            let id = id as u32;
            let parts: Vec<&str> = want.split('|').collect();
            let mut check = |what: &str, got: String, want: &str| {
                if got != want {
                    failures.push(format!(
                        "seed {seed} column ({x}, {z}) node {id} {what} {:?}: rust {got} game {want}",
                        tree.nodes[id as usize]
                    ));
                }
            };
            let ctx = EvalContext::caching();
            if parts[0] != "x" {
                check("layer", volume_hash(s, &ctx, id, &layer), parts[0]);
            }
            let points: Vec<String> = ys
                .iter()
                .enumerate()
                .map(|(i, &y)| {
                    let v = s.sample_value(&ctx, id, x + i as i32, y, z - i as i32);
                    format!("{:x}", v.to_bits())
                })
                .collect();
            check("points", points.join("/"), parts[1]);
            if parts[2] != "x" {
                check("column", volume_hash(s, &ctx, id, &column), parts[2]);
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} node mismatches, first ones:\n{}",
        failures.len(),
        failures
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
