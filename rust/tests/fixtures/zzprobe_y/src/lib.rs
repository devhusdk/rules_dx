pub fn value() -> i32 {
    42
}

#[cfg(test)]
mod tests {
    #[test]
    fn value_is_42() {
        assert_eq!(super::value(), 42);
    }
}
