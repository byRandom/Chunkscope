//! Bit-exact reimplementations of the JDK behaviour that Minecraft: Java
//! Edition depends on. Every function here is verified against a real JDK
//! (see `parity/` at the workspace root).

mod digit_table;
pub mod random;
pub mod stream;
pub mod strict_math;
pub mod string;

pub use random::{BitRandom, JavaRandom};
