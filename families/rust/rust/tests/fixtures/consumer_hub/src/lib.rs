use greeting::greeting;

pub fn message() -> String {
    greeting()
}

#[cfg(test)]
mod tests {
    use super::message;

    #[test]
    fn resolves_the_consumer_hub_crate() {
        assert_eq!(message(), "Hello from the consumer hub");
    }
}
