//! Noise generators, bit-exact with Minecraft 26.x (`levelgen.synth`).
//!
//! Since 26.1 the game evaluates noise in `float` precision. A port built on
//! older versions (all `double`) gives different results.

mod normal;
mod perlin;
pub mod vanilla;

pub use normal::{Layer, NoiseParameters, NoiseStack, NormalNoise, Normalization, OctaveInfo};
pub use perlin::{PerlinNoise, wrap};
