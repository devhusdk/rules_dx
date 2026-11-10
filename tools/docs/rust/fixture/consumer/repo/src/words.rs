//! Words a greeter puts together.

/// A greeter that remembers its salutation.
pub struct Greeter {
    salutation: String,
}

impl Greeter {
    /// Greets one name with the remembered salutation.
    pub fn greet(&self, name: &str) -> String {
        format!("{}, {name}", self.salutation)
    }
}
