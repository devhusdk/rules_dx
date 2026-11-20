use super::*;

const PCS: &str = "0x10 1\n0x20 0\n0x30 0\n0x40 1\n";

const SYMBOLIZED: &str = "0x10\nm.greet\n/w/src/greet.zig:2:5\n\n\
0x20\nm.greet\n/w/src/greet.zig:3:5\n\n\
0x30\nm.inner\n/w/src/greet.zig:3:9\nm.greet\n/w/src/greet.zig:2:5\n\n\
0x40\nstd.mem.eql\n??:0:0\n\n";

#[test]
fn marks_a_line_run_when_any_pc_on_it_ran() {
    assert_eq!(
        sancov_to_lcov(PCS, SYMBOLIZED),
        Ok("SF:/w/src/greet.zig\nDA:2,1\nDA:3,0\nLF:2\nLH:1\nend_of_record\n".to_string())
    );
}

#[test]
fn keeps_windows_drive_colons_in_paths() {
    let out = sancov_to_lcov("0x1 1\n", "0x1\nf\r\nC:\\w\\a.zig:7:1\r\n\r\n").expect("converts");
    assert_eq!(out, "SF:C:\\w\\a.zig\nDA:7,1\nLF:1\nLH:1\nend_of_record\n");
}

#[test]
fn rejects_malformed_input() {
    for (pcs, symbolized) in [
        ("0x1\n", ""),
        ("10 1\n", ""),
        ("0x1 2\n", ""),
        ("0x1 1 extra\n", ""),
        ("0x1 1\n", "f\na.zig:1:1\n"),
        ("0x1 1\n", "0x2\nf\na.zig:1:1\n"),
        ("0x1 1\n", "0x1\nf\n"),
        ("0x1 1\n", "0x1\nf\na.zig\n"),
    ] {
        let error = sancov_to_lcov(pcs, symbolized).expect_err(pcs);
        assert!(
            error.to_string().starts_with("malformed sancov input"),
            "{error}"
        );
    }
}

#[test]
fn writes_nothing_for_no_pcs() {
    assert_eq!(sancov_to_lcov("", ""), Ok(String::new()));
}
