pub const BAZEL_FLAG: &str = "--bazel";

pub const PRUNE_UNOBSERVED_FLAG: &str = "--prune-unobserved";

pub const RECOVERY_GUIDANCE: &str = "bazel clean forwarded; managed links may now dangle: \
    re-run `dx env` and `dx codegen` to repair the selection";

pub fn bazel_forward_argv() -> Vec<String> {
    vec!["clean".to_owned()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_shape_is_frozen() {
        assert_eq!(BAZEL_FLAG, "--bazel");
        assert_eq!(PRUNE_UNOBSERVED_FLAG, "--prune-unobserved");
        assert_eq!(bazel_forward_argv(), vec!["clean".to_owned()]);
        assert!(RECOVERY_GUIDANCE.contains("dx env"));
    }
}
