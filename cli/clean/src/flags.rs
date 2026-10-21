pub const PRUNE_UNOBSERVED_FLAG: &str = "--prune-unobserved";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_shape_is_frozen() {
        assert_eq!(PRUNE_UNOBSERVED_FLAG, "--prune-unobserved");
    }
}
