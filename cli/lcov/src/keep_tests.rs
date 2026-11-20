use super::*;

#[test]
fn keeps_listed_sources_under_their_listed_path() {
    let lcov = "SF:/x/execroot/_main/src/a.zig\nDA:1,1\nend_of_record\n\
                SF:/x/lib/std/mem.zig\nDA:2,0\nend_of_record\n\
                SF:src/b.zig\nDA:3,1\nend_of_record\n";
    assert_eq!(
        keep_sources(lcov, &["src/a.zig", "src/b.zig"]),
        "SF:src/a.zig\nDA:1,1\nend_of_record\nSF:src/b.zig\nDA:3,1\nend_of_record\n"
    );
}

#[test]
fn keeps_exact_match_over_shorter_suffix() {
    let lcov = "SF:tools/main.zig\nDA:1,1\nend_of_record\nSF:main.zig\nDA:2,0\nend_of_record\n";
    assert_eq!(keep_sources(lcov, &["main.zig", "tools/main.zig"]), lcov);
}

#[test]
fn prefers_longest_suffix_in_any_order() {
    let lcov = "SF:/x/_main/tools/main.zig\nDA:1,1\nend_of_record\n\
                SF:/x/_main/main.zig\nDA:2,0\nend_of_record\n";
    let want = "SF:tools/main.zig\nDA:1,1\nend_of_record\nSF:main.zig\nDA:2,0\nend_of_record\n";
    assert_eq!(keep_sources(lcov, &["main.zig", "tools/main.zig"]), want);
    assert_eq!(keep_sources(lcov, &["tools/main.zig", "main.zig"]), want);
}
