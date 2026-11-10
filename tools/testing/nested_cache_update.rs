//! Re-records the cache sets and locks of nested tests: `bazel run //tools/testing:nested_cache_update -- <test label>...`.

use std::path::PathBuf;

const DIR: &str = "tools/testing/nested_cache";

fn fail(message: &str) -> ! {
    eprintln!("nested_cache_update: {message}");
    std::process::exit(1);
}

/// Splits `//pkg:name` into its package and name.
fn split_label(label: &str) -> Option<(&str, &str)> {
    let rest = label.strip_prefix("//")?;
    let (package, name) = rest.split_once(':')?;
    (!name.is_empty() && !name.contains('/')).then_some((package, name))
}

fn main() {
    let workspace = std::env::var_os("BUILD_WORKSPACE_DIRECTORY")
        .map(PathBuf::from)
        .unwrap_or_else(|| fail("run with `bazel run //tools/testing:nested_cache_update -- <test label>...`"));
    let labels: Vec<String> = std::env::args().skip(1).collect();
    if labels.is_empty() {
        fail("pass the labels of the nested tests to record");
    }
    let parts: Vec<(&str, &str)> = labels
        .iter()
        .map(|label| split_label(label).unwrap_or_else(|| fail(&format!("{label} is not a //package:name label"))))
        .collect();
    let test_command = format!(
        "bazel test {} --test_env=DX_NESTED_BAZEL_NETWORK=1 --nocache_test_results --nozip_undeclared_test_outputs --test_tag_filters= --build_tag_filters=",
        labels.join(" ")
    );
    let target = workspace.join(DIR);
    for (package, name) in parts {
        let recorded = workspace
            .join("bazel-testlogs")
            .join(package)
            .join(name)
            .join("test.outputs")
            .join("nested_cache");
        let entries = std::fs::read_dir(&recorded)
            .unwrap_or_else(|error| fail(&format!("{} recorded nothing: {error}; run `{test_command}` first", recorded.display())));
        for entry in entries.filter_map(Result::ok) {
            let dst = target.join(entry.file_name());
            std::fs::copy(entry.path(), &dst)
                .unwrap_or_else(|error| fail(&format!("cannot write {}: {error}", dst.display())));
            println!("wrote {DIR}/{}", entry.file_name().to_string_lossy());
        }
    }
}
