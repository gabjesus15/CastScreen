//! Hardware-accelerated video encoding and high-fidelity audio encoding.

pub mod aac;
pub mod nvenc;

pub use aac::{decode_pcm_payload, PcmPackError, PcmPacker};
pub use nvenc::{NvencEncoder, NvencError};
