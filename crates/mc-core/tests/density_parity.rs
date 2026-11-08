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
