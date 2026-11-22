#![deny(clippy::indexing_slicing, clippy::string_slice)]

pub fn greeting() -> &'static str {
    "hello"
}
