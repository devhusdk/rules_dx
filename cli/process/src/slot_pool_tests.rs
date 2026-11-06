use super::*;

const GIB: u64 = 1 << 30;

#[test]
fn the_reserve_is_empty_for_no_servers() {
    for memory in [8 * GIB, 16 * GIB, 64 * GIB] {
        assert!(reserve_range(memory, 0).is_empty());
    }
}

#[test]
fn the_reserve_range_matches_the_hand_computed_bounds() {
    assert_eq!(reserve_range(8 * GIB, 3), 1..2);
    assert_eq!(reserve_range(16 * GIB, 3), 3..5);
    assert_eq!(reserve_range(64 * GIB, 3), 16..16);
    assert_eq!(reserve_range(64 * GIB, 8), 16..16);
}

#[test]
fn two_holders_share_the_reserve() {
    let scratch = dx_test_scratch::scratch("dx-slot-pool-shared-");
    let first = hold_reserve(scratch.path(), 0..3);
    let second = hold_reserve(scratch.path(), 1..3);
    assert_eq!((first.held.len(), first.busy.len()), (3, 0));
    assert_eq!((second.held.len(), second.busy.len()), (2, 0));
    assert!(matches!(
        lock_slot(&slot_path(scratch.path(), 1), false),
        Ok(None)
    ));
}

#[test]
fn an_exclusively_held_index_is_busy_and_not_waited_on() {
    let scratch = dx_test_scratch::scratch("dx-slot-pool-busy-");
    let _held = lock_slot(&slot_path(scratch.path(), 0), false)
        .expect("lock")
        .expect("free");
    let reserve = hold_reserve(scratch.path(), 0..2);
    assert_eq!(reserve.busy, vec![0]);
    assert_eq!(reserve.held.len(), 1);
}

#[cfg(unix)]
#[test]
fn a_read_only_slot_file_still_takes_a_shared_lock() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = dx_test_scratch::scratch("dx-slot-pool-readonly-");
    let path = slot_path(scratch.path(), 0);
    std::fs::write(&path, "").expect("file");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).expect("chmod");
    let reserve = hold_reserve(scratch.path(), 0..1);
    assert_eq!((reserve.held.len(), reserve.busy.len()), (1, 0));
}

#[test]
fn slot_counts_follow_memory() {
    assert_eq!(slot_count(0, 0), 1);
    assert_eq!(slot_count(16 * GIB, 6 * GIB), 3);
    assert_eq!(slot_count(u64::MAX, 0), MAX_SLOTS);
}

#[test]
fn the_slot_directory_sits_under_the_user_cache() {
    let env = |name: &str| (name == "HOME").then(|| "/home/someone".to_string());
    if let Some(dir) = slots_dir(&env) {
        assert!(dir.ends_with("dx/slots") || dir.ends_with("dx\\slots"));
    }
    assert!(slots_dir(&|_: &str| None).is_none());
}

#[test]
fn meminfo_text_gives_total_bytes() {
    assert_eq!(
        parse_meminfo("MemFree: 1 kB\nMemTotal:       2048 kB\n"),
        Some(2048 * 1024)
    );
    assert_eq!(parse_meminfo("MemFree: 1 kB\n"), None);
    assert_eq!(parse_meminfo("MemTotal: lots kB\n"), None);
    assert_eq!(parse_meminfo("MemTotal:\n"), None);
    assert_eq!(parse_meminfo(&format!("MemTotal: {} kB\n", u64::MAX)), None);
}

#[test]
fn cgroup_limits_ignore_unlimited_and_junk() {
    assert_eq!(parse_cgroup_limit("1073741824\n"), Some(1 << 30));
    assert_eq!(parse_cgroup_limit("max\n"), None);
    assert_eq!(parse_cgroup_limit("9223372036854771712"), None);
}

#[test]
fn the_tightest_limit_up_the_cgroup_tree_wins() {
    let scratch = dx_test_scratch::scratch("dx-slot-pool-cgroup-v2-");
    let root = scratch.path();
    let leaf = root.join("a").join("b");
    std::fs::create_dir_all(&leaf).expect("dirs");
    std::fs::write(root.join("a").join("memory.max"), "4000\n").expect("parent");
    std::fs::write(leaf.join("memory.max"), "9000\n").expect("leaf");
    assert_eq!(cgroup_limit_in("0::/a/b\n", root), Some(4000));
    std::fs::write(leaf.join("memory.max"), "max\n").expect("leaf");
    assert_eq!(cgroup_limit_in("0::/a/b\n", root), Some(4000));
    std::fs::write(root.join("a").join("memory.max"), "max\n").expect("parent");
    assert_eq!(cgroup_limit_in("0::/a/b\n", root), None);
}

#[test]
fn a_v1_memory_controller_limit_combines_with_v2() {
    let scratch = dx_test_scratch::scratch("dx-slot-pool-cgroup-v1-");
    let root = scratch.path();
    let group = root.join("memory").join("g");
    std::fs::create_dir_all(&group).expect("dirs");
    std::fs::write(group.join("memory.limit_in_bytes"), "5000\n").expect("v1");
    let own = "4:cpu:/x\n3:memory:/g\n";
    assert_eq!(cgroup_limit_in(own, root), Some(5000));
    std::fs::write(root.join("memory.max"), "3000\n").expect("v2");
    let both = "0::/\n3:cpu,memory:/g\n";
    assert_eq!(cgroup_limit_in(both, root), Some(3000));
    assert_eq!(cgroup_limit_in("", root), Some(3000));
}

#[cfg(target_os = "linux")]
#[test]
fn available_memory_reports_something_on_linux() {
    assert!(available_memory().is_some_and(|bytes| bytes > 0));
}

#[test]
fn an_empty_range_holds_nothing() {
    let scratch = dx_test_scratch::scratch("dx-slot-pool-empty-");
    let reserve = hold_reserve(scratch.path(), 2..2);
    assert!(reserve.held.is_empty() && reserve.busy.is_empty());
}

#[test]
fn an_unusable_directory_makes_every_index_busy() {
    let scratch = dx_test_scratch::scratch("dx-slot-pool-nodir-");
    let blocker = scratch.path().join("file");
    std::fs::write(&blocker, "").expect("file");
    let reserve = hold_reserve(&blocker.join("slots"), 0..3);
    assert_eq!(reserve.busy, vec![0, 1, 2]);
}

#[test]
fn a_missing_or_non_file_slot_keeps_the_open_error() {
    let scratch = dx_test_scratch::scratch("dx-slot-pool-odd-");
    let missing = slot_path(scratch.path(), 0);
    assert!(lock_slot(&missing, true).is_ok());
    let dir = slot_path(scratch.path(), 1);
    std::fs::create_dir(&dir).expect("dir");
    assert!(lock_slot(&dir, false).is_err());
}
