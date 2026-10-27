//! Free-disk thresholds shared by the worktree reclaim and the Bazel host-failure diagnosis.

/// Free bytes under which a disk counts as low, together with `LOW_DISK_PERCENT`.
pub(crate) const LOW_DISK_BYTES: u64 = 20 * 1024 * 1024 * 1024;
/// Free percentage under which a disk counts as low, together with `LOW_DISK_BYTES`.
pub(crate) const LOW_DISK_PERCENT: u64 = 10;
/// Free bytes under which a disk counts as full, whatever its size.
pub(crate) const DISK_FULL_BYTES: u64 = 1 << 30;

/// Whether `free` of `total` bytes is under 20 GiB and under 10%, or full.
///
/// A full disk always counts as low, so reclaim runs on a small disk before `add` refuses.
pub(crate) fn is_low(free: u64, total: u64) -> bool {
    is_full(free)
        || (free < LOW_DISK_BYTES
            && u128::from(free) * 100 < u128::from(total) * u128::from(LOW_DISK_PERCENT))
}

/// Whether `free` bytes is under 1 GiB; the percentage rule does not apply.
pub(crate) fn is_full(free: u64) -> bool {
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
