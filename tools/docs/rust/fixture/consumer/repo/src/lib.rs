//! Greetings a consumer of rules_dx documents with its own shard.

pub mod words;

pub use words::Greeter;

/// Greets one name with a fixed salutation.
pub fn greet(name: &str) -> String {
    format!("{}, {name}", salutation())
}

fn salutation() -> &'static str {
    "Hello"
}
