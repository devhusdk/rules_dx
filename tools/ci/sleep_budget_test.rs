use std::collections::BTreeMap;
use std::io::Write as _;

use dx_guard::allowlist::{Counts, add, parse, render};
use dx_guard::snapshot::read_input;
use sleep_budget::{
    BUDGET, INPUTS, KIND, MIN_FILES, UPDATE, found, inline_test_lines, line_sleeps, lowered,
    over_budget, scanned, sleeps, test_path, under_budget,
};
use starlark_syntax as _;
use yaml_serde as _;

fn code(text: &str) -> String {
    text.replace('Z', "sleep").replace('Q', "timeout")
}

#[test]
fn no_test_file_goes_over_its_sleep_budget() {
    let mut files = BTreeMap::new();
    for variable in INPUTS {
        let part = serde_json::from_str::<BTreeMap<String, String>>(&read_input(variable));
        assert!(part.is_ok(), "{variable}: {part:?}");
        files.extend(part.unwrap_or_default());
    }
    let files = scanned(files);
    assert!(
        files.len() >= MIN_FILES,
        "only {} files scanned",
        files.len()
    );
    let found = found(&files);
    let (budget, mut problems) = parse(&read_input("SLEEP_BUDGET"), &[KIND]);
    let lower = under_budget(&found, &budget);
    if !lower.is_empty() {
        let written = writeln!(
            std::io::stderr(),
            "{}\nThese files are under their budget in {BUDGET}. Lower it with `{UPDATE}`.",
            lower.join("\n")
        );
        assert!(written.is_ok(), "{written:?}");
    }
    problems.extend(over_budget(&found, &budget));
    assert!(
        problems.is_empty(),
        "{}\nWait on the event the sleep stands in for with a deadline, or set the time \
         explicitly. Never raise a budget; `{UPDATE}` only lowers it.",
        problems.join("\n")
    );
}

#[test]
fn counts_a_sleep_in_every_language() {
    for line in [
        "std::thread::Z(Duration::from_millis(5));",
        "tokio::time::Z(d).await;",
        "Z(Duration::from_secs(1));",
        "time.Z(0.1)",
        "await asyncio.Z(1)",
        "time.Z(time.Second)",
        "Thread.Z(100);",
        "Process.Z(10)",
        "timer:Z(20),",
        "std::this_thread::Z_for(1s);",
        "tokio::time::Z_until(deadline).await;",
        "Z 1",
        "while Z 30; do",
        "  Z \"$delay\"",
        "Z $SECONDS",
        "Start-Z -Seconds 1",
        "[Threading.Thread]::Z(2000)",
        "Q /t 5",
        "let child = Command::new(\"Z\").arg(\"5\").spawn();",
        "subprocess.Popen([\"Z\", \"5\"])",
        "script = \"#!/bin/sh\\nZ 5\\n\"",
    ] {
        assert_eq!(line_sleeps(&code(line)), 1, "{line}");
    }
    assert_eq!(line_sleeps(&code("Z 1; Z 2")), 2);
}

#[test]
fn skips_names_that_only_contain_the_word() {
    for line in [
        "fn Z_until_gate(gate: &Path) {}",
        "fn Z(&self, duration: Duration) {}",
        "def Z(self, seconds):",
        "func Z(d time.Duration) {}",
        "let Zs = 3;",
        "no_Z(x);",
        ".expect(\"Z\");",
        "monkeypatch.setattr(update.time, \"Z\", lambda _: None)",
        "--Z-ms 5",
        "a Z budget",
    ] {
        assert_eq!(line_sleeps(&code(line)), 0, "{line}");
    }
}

#[test]
fn finds_test_paths() {
    for path in [
        "cli/process/src/lifecycle_tests.rs",
        "tools/ci/comment_budget_test.rs",
        "cli/cli/tests/agents/tests/dev_launcher.rs",
        "cli/cli/src/test_support.rs",
        "cli/worktree/src/tests.rs",
        "cli/test_scratch/src/lib.rs",
        "tools/testing/nested/src/lib.rs",
        "core/quality/artifacts/update_test/update_test.py",
        "android/tests/fixtures/rust_apk/device_run.sh",
        "erlang/tests/fixtures/erlang_app/store/test/kv_store_SUITE.erl",
        "astro/tests/fixtures/app/site_island.test.js",
        "python/conftest.py",
    ] {
        assert!(test_path(path), "{path}");
    }
    for path in [
        "cli/process/src/lifecycle.rs",
        "tools/dx",
        "tools/ci/BUILD.bazel",
        "core/quality/artifacts/update.py",
        ".github/workflows/evidence.yml",
        "api_spec/defs.bzl",
    ] {
        assert!(!test_path(path), "{path}");
    }
}

#[test]
fn counts_only_the_test_items_of_a_production_rust_file() {
    let text = code(
        "fn retry() {\n    std::thread::Z(BACKOFF);\n}\n\n\
         #[cfg(test)]\n#[path = \"a_tests.rs\"]\nmod a_tests;\n\n\
         #[cfg(not(test))]\nfn live() { Z(D); }\n\n\
         #[cfg(test)]\nfn helper(\n    a: u8,\n) {\n    Z(D);\n}\n\n\
         fn after() { Z(D); }\n\n\
         #[cfg(test)]\nmod tests {\n    #[test]\n    fn waits() {\n        if a {\n            Z(D);\n        }\n    }\n}\n",
    );
    assert_eq!(inline_test_lines(&text).len(), 13);
    assert_eq!(sleeps("cli/a/src/lib.rs", &text), 2);
    assert_eq!(sleeps("cli/a/src/lib_tests.rs", &text), 5);
    let whole = code("#![cfg(test)]\n\nfn a() { Z(D); }\n");
    assert_eq!(sleeps("cli/a/src/probe.rs", &whole), 1);
    assert_eq!(sleeps("cli/a/src/run.py", &code("time.Z(1)\n")), 0);
}

fn counts(entries: &[(&str, usize)]) -> Counts {
    let mut counts = Counts::new();
    for (path, count) in entries {
        add(&mut counts, path, KIND, *count);
    }
    counts
}

#[test]
fn a_new_sleep_fails_and_an_unlisted_file_has_no_budget() {
    let found = counts(&[("cli/a_test.rs", 2), ("cli/new_test.rs", 1)]);
    let budget = counts(&[("cli/a_test.rs", 1), ("cli/gone_test.rs", 3)]);
    assert_eq!(
        over_budget(&found, &budget),
        [
            "cli/a_test.rs: 2 sleeps, budget 1",
            "cli/new_test.rs: 1 sleeps, budget 0",
        ]
    );
    assert_eq!(
        under_budget(&found, &budget),
        ["cli/gone_test.rs: 0 sleeps, budget 3"]
    );
}

#[test]
fn update_only_lowers_or_drops_lines() {
    let found = counts(&[
        ("cli/a_test.rs", 1),
        ("cli/b_test.rs", 5),
        ("cli/new_test.rs", 2),
    ]);
    let budget = counts(&[
        ("cli/a_test.rs", 4),
        ("cli/b_test.rs", 2),
        ("cli/gone_test.rs", 3),
    ]);
    assert_eq!(
        render(&lowered(&found, &budget)),
        format!("cli/a_test.rs {KIND} 1\ncli/b_test.rs {KIND} 2\n")
    );
}
