pub(crate) const LOW_DISK_BYTES: u64 = 20 * 1024 * 1024 * 1024;
pub(crate) const LOW_DISK_PERCENT: u64 = 10;
pub(crate) const DISK_FULL_BYTES: u64 = 1 << 30;

pub(crate) fn is_low(free: u64, total: u64) -> bool {
    is_full(free)
        || (free < LOW_DISK_BYTES
            && u128::from(free) * 100 < u128::from(total) * u128::from(LOW_DISK_PERCENT))
}

#[must_use]
pub fn is_full(free: u64) -> bool {
    free < DISK_FULL_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_needs_both_limits_and_full_needs_only_one() {
        let gib = 1u64 << 30;
        assert!(is_low(5 * gib, 100 * gib));
        assert!(!is_low(5 * gib, 20 * gib));
        assert!(!is_low(30 * gib, 1000 * gib));
        assert!(is_low(gib - 1, 8 * gib), "a full small disk is low");
        assert!(is_full(gib - 1));
        assert!(!is_full(gib));
    }
}
