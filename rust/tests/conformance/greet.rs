//! Greeting fixture library.

/// Returns the greeting for `name`.
pub fn greet(name: &str) -> String {
    format!("hello {name}")
}
