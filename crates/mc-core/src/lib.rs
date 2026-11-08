//! Minecraft: Java Edition world-seed logic.

pub mod random;
pub mod seed;
pub mod slime;

pub use seed::parse_seed;
pub use slime::is_slime_chunk;
