fn in_shard() {
    assert_eq!(std::env::var("TEST_TOTAL_SHARDS").as_deref(), Ok("3"));
    let index: usize = std::env::var("TEST_SHARD_INDEX").unwrap().parse().unwrap();
    assert!(index < 3);
}

#[test]
fn first() {
    in_shard();
}

#[test]
fn second() {
    in_shard();
}

#[test]
fn third() {
    in_shard();
}

#[test]
fn fourth() {
    in_shard();
}
