//! Noise parity against the real game classes
//! (parity/game-oracle/GameOracle.java).

use std::borrow::Cow;

use mc_core::noise::{NoiseParameters, NormalNoise, Normalization, PerlinNoise, vanilla};
use mc_core::random::{LegacyRandomSource, RandomSource, XoroshiroRandomSource};

const PERLIN: &str = include_str!("../../../parity/fixtures/game/perlin.txt");
const NORMAL_OCTAVES: &str = include_str!("../../../parity/fixtures/game/normal_octaves.txt");
const NOISE: &str = include_str!("../../../parity/fixtures/game/noise.txt");

fn f32_bits(f: f32) -> String {
    format!("{:x}", f.to_bits())
}

fn f64_bits(f: f64) -> String {
    format!("{:x}", f.to_bits())
}

fn parse_f64_bits(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).unwrap())
}

fn parse_point(s: &str) -> (f64, f64, f64) {
    let v: Vec<f64> = s.split(',').map(|c| c.parse().unwrap()).collect();
    (v[0], v[1], v[2])
}

#[test]
fn perlin_noise_matches_game() {
    for line in PERLIN.lines() {
        let mut tokens = line.split(' ');
        let kind = tokens.next().unwrap();
        let seed: i64 = tokens.next().unwrap().parse().unwrap();
        let (noise, next) = match kind {
            "xoroshiro" => {
                let mut r = XoroshiroRandomSource::new(seed);
                (PerlinNoise::new(&mut r), r.next_long())
            }
            "legacy" => {
                let mut r = LegacyRandomSource::new(seed);
                (PerlinNoise::new(&mut r), r.next_long())
            }
            other => panic!("unknown kind {other}"),
        };
        assert_eq!(
            format!("next={next}"),
            tokens.next().unwrap(),
            "{kind} {seed}: RNG state after construction"
        );
        for sample in tokens {
            let (point, want) = sample.split_once('=').unwrap();
            let (x, y, z) = parse_point(point);
            let got = format!(
                "{}/{}",
                f32_bits(noise.get(x, y, z)),
                f32_bits(noise.get_2d(x, z))
            );
            assert_eq!(got, want, "{kind} seed {seed} at ({x}, {y}, {z})");
        }
    }
}

#[test]
fn normal_noise_octaves_match_game() {
    for line in NORMAL_OCTAVES.lines() {
        let t: Vec<&str> = line.split(' ').collect();
        let name = t[0];
        let mods: Vec<f64> = t[5]
            .trim_matches(|c| c == '[' || c == ']')
            .split(',')
            .filter(|s| !s.is_empty())
            .map(parse_f64_bits)
            .collect();
        let params = NoiseParameters {
            base_amplitude: parse_f64_bits(t[1]),
            base_octave: t[2].parse().unwrap(),
            octave_count: t[3].parse().unwrap(),
            normalize: match t[4] {
                "DISABLED" => Normalization::Disabled,
                "ENABLED" => Normalization::Enabled,
                "LEGACY" => Normalization::Legacy,
                other => panic!("unknown normalization {other}"),
            },
            amplitude_modifiers: Cow::Owned(mods),
        };
        if let Some((_, table)) = vanilla::ALL.iter().find(|(n, _)| *n == name) {
            assert_eq!(
                *table, params,
                "{name}: generated table differs from the game's decoded parameters"
            );
        }

        let noise = NormalNoise::new(&params);
        let mut got = format!("factor={}", f64_bits(noise.normalization_factor()));
        for o in noise.octaves() {
            got.push_str(&format!(
                " {}:{}:{}",
                o.octave_index,
                f64_bits(o.frequency),
                f64_bits(o.amplitude)
            ));
        }
        let stack = NormalNoise::instantiate(42, name, &params);
        got.push_str(&format!(
            " sample={}/{}",
            f32_bits(stack.get(12.5, -40.0, 7_000.75)),
            f32_bits(stack.get_2d(-3.25, 99.0))
        ));
        assert_eq!(got, t[6..].join(" "), "{name}");
    }
}

#[test]
fn every_vanilla_noise_matches_game() {
    let mut lines = NOISE.lines();
    let points: Vec<(f64, f64, f64)> = lines
        .next()
        .unwrap()
        .strip_prefix("points ")
        .unwrap()
        .split(' ')
        .map(parse_point)
        .collect();
    let mut checked = 0;
    for line in lines {
        let mut t = line.split(' ');
        let seed: i64 = t.next().unwrap().parse().unwrap();
        let name = t.next().unwrap();
        let (_, params) = vanilla::ALL.iter().find(|(n, _)| *n == name).expect(name);
        let noise = NormalNoise::instantiate(seed, name, params);
        for (&(x, y, z), want) in points.iter().zip(t) {
            let got = format!(
                "{}/{}",
                f32_bits(noise.get(x, y, z)),
                f32_bits(noise.get_2d(x, z))
            );
            assert_eq!(got, want, "{name} seed {seed} at ({x}, {y}, {z})");
            checked += 1;
        }
    }
    assert_eq!(checked, 6 * vanilla::ALL.len() * points.len());
}
