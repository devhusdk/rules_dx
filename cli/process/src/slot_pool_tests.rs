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
