//! Shapes and lengths for the docs site API reference.

pub mod shapes;

pub use shapes::Circle;

/// Adds two lengths and a carry, wrapping on overflow.
pub fn add(left: u64, right: u64, carry: u32) -> u64 {
    left.wrapping_add(private_offset(right))
        .wrapping_add(u64::from(carry))
}

fn private_offset(value: u64) -> u64 {
    value
}

include!(concat!(env!("OUT_DIR"), "/generated.rs"));
