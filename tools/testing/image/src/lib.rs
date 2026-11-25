#![deny(clippy::indexing_slicing, clippy::string_slice)]

pub mod fatbin;

pub use dx_binfmt::{inspect, machine};
