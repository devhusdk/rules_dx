use std::path::{Path, PathBuf};

fn cobertura(dir: &Path) -> Option<PathBuf> {
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = cobertura(&path) {
                return Some(found);
            }
        } else if path.file_name().is_some_and(|name| name == "cobertura.xml") {
            return Some(path);
        }
    }
    None
}

fn report(kcov: &Path, args: &[&str], out: &Path, bin: &Path) -> String {
    let output = std::process::Command::new(kcov)
        .args(args)
        .arg(out)
        .arg(bin)
        .output()
        .unwrap_or_else(|error| panic!("{}: {error}", kcov.display()));
    assert!(
        output.status.success(),
        "kcov failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let found =
        cobertura(out).unwrap_or_else(|| panic!("no cobertura.xml under {}", out.display()));
    dx_testing::read_text(&found)
}

fn tools() -> (PathBuf, PathBuf) {
    let kcov = std::path::absolute(dx_testing::env_file("DX_KCOV")).expect("kcov resolves");
    let bin = std::path::absolute(dx_testing::env_file("DX_KCOV_SMOKE")).expect("smoke resolves");
    (kcov, bin)
}

#[test]
fn kcov_reports_a_covered_line() {
    let (kcov, bin) = tools();
    let text = report(&kcov, &[], &dx_testing::scratch_dir("kcov_out"), &bin);
    let covered = text
        .split("hits=\"")
        .skip(1)
        .any(|rest| rest.starts_with(|c: char| ('1'..='9').contains(&c)));
    assert!(covered, "no covered line in\n{text}");
}

#[test]
fn lazy_prefix_keeps_the_longest_source() {
    let (kcov, bin) = tools();
    let cwd = std::env::current_dir().expect("the test runs in a directory");
    let replace = format!(
        "--replace-src-path=^.*?/(?=(smoke.c|tools/coverage/kcov/smoke.c)$):{}/",
        cwd.display()
    );
    let text = report(
        &kcov,
        &[&replace],
        &dx_testing::scratch_dir("kcov_out_lazy"),
        &bin,
    );
    let kept = text
        .split("filename=\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .any(|name| name.ends_with("tools/coverage/kcov/smoke.c"));
    assert!(kept, "lazy prefix did not keep the longest source\n{text}");
}
