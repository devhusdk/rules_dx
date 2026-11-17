//! The aarch64 builtins archive keeps one lse_<pat>_<size>_<model>.o per atomic helper.

fn digits(bytes: &[u8], from: usize) -> usize {
    bytes[from..]
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count()
}

fn member_at(bytes: &[u8], start: usize) -> Option<usize> {
    let mut at = start + b"lse_".len();
    at += bytes[at..]
        .iter()
        .take_while(|byte| byte.is_ascii_lowercase())
        .count();
    if bytes.get(at) != Some(&b'_') {
        return None;
    }
    at += 1 + digits(bytes, at + 1);
    if bytes.get(at) != Some(&b'_') {
        return None;
    }
    at += 1 + digits(bytes, at + 1);
    bytes[at..]
        .starts_with(b".pic.o")
        .then_some(at + b".pic.o".len())
}

fn members(bytes: &[u8]) -> std::collections::BTreeSet<Vec<u8>> {
    let mut found = std::collections::BTreeSet::new();
    let mut at = 0;
    while at + 4 <= bytes.len() {
        if bytes[at..].starts_with(b"lse_") {
            if let Some(end) = member_at(bytes, at) {
                found.insert(bytes[at..end].to_vec());
                at = end;
                continue;
            }
        }
        at += 1;
    }
    found
}

#[test]
fn the_archive_has_every_lse_helper() {
    let archive = dx_testing::env_file("DX_AARCH64_BUILTINS");
    let bytes =
        std::fs::read(&archive).unwrap_or_else(|error| panic!("{}: {error}", archive.display()));
    let count = members(&bytes).len();
    assert_eq!(
        count,
        125,
        "{} has {count} lse_*.o members, want 125",
        archive.display()
    );
}
