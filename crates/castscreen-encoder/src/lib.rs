//! Hardware-accelerated video encoding and high-fidelity audio encoding.

pub mod aac;
pub mod nvenc;

pub use aac::{AacEncodeError, AacEncoder};
pub use nvenc::{NvencEncoder, NvencError};
