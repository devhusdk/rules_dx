use std::collections::BTreeSet;
use std::path::PathBuf;

use dx_release_tools::cell::{parse_matrix, Cell};

fn data(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a test input"));
    dx_testing::resolve_runfiles(&rel)
}

fn host_cell() -> (Cell, PathBuf) {
    let matrix = std::fs::read_to_string(data("DX_RELEASE_MATRIX")).expect("release matrix");
    let cells = parse_matrix(&matrix).expect("release-matrix/v1");
    assert_eq!(cells.len(), 5, "{cells:?}");
    let archive = data("DX_HOST_CELL");
    let base = archive
        .file_name()
        .and_then(|name| name.to_str())
        .expect("archive name")
        .to_owned();
    let cell = cells
        .into_iter()
        .find(|cell| cell.archive_name() == base)
        .unwrap_or_else(|| panic!("{base} names no release cell"));
    (cell, archive)
}

#[test]
fn host_cell_ships_exactly_its_members() {
    let (cell, archive) = host_cell();
    let listing = dx_deploy_tools::archive_listing(&archive).expect("archive listing");
    let have: BTreeSet<String> = listing.iter().map(|(name, _, _)| name.clone()).collect();
    let mut want: BTreeSet<String> = ["NOTICE", "IDENTITY.txt"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    want.insert(cell.exe.clone());
    want.extend(cell.runtime.iter().cloned());
    assert_eq!(have, want);
    for (name, mode, _) in &listing {
        let expected = if *name == cell.exe { 0o755 } else { 0o644 };
        assert_eq!(*mode, expected, "{name}");
    }
}

#[test]
fn host_cell_notice_lists_shipped_crates_only() {
    let (_, archive) = host_cell();
    let notice = dx_deploy_tools::archive_read_member(&archive, "NOTICE").expect("NOTICE");
    let notice = String::from_utf8(notice).expect("UTF-8 NOTICE");
    assert!(notice.contains("\n=== serde "), "serde is shipped");
    assert!(
        !notice.contains("\n=== serde_derive "),
        "proc macros are not shipped"
    );
}
