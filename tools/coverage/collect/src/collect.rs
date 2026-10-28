use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const RAW_MAGIC_64: u64 = 0xff6c_7072_6f66_7281;
const RAW_MAGIC_32: u64 = 0xff6c_7072_6f66_5281;
const RUNTIME_OBJECTS_SUFFIX: &str = "runtime_objects_list.txt";

pub fn raw_profile_version(header: &[u8]) -> Option<u32> {
    let magic = u64::from_le_bytes(header.get(0..8)?.try_into().ok()?);
    let version = u64::from_le_bytes(header.get(8..16)?.try_into().ok()?);
    if magic == RAW_MAGIC_64 || magic == RAW_MAGIC_32 {
        return Some(low_bits(version));
    }
    let swapped = magic.swap_bytes();
    if swapped == RAW_MAGIC_64 || swapped == RAW_MAGIC_32 {
        return Some(low_bits(version.swap_bytes()));
    }
    None
}

fn low_bits(version: u64) -> u32 {
    (version & 0xffff_ffff) as u32
}

pub fn group_by_version(files: Vec<(PathBuf, u32)>) -> BTreeMap<u32, Vec<PathBuf>> {
    let mut groups: BTreeMap<u32, Vec<PathBuf>> = BTreeMap::new();
    for (path, version) in files {
        groups.entry(version).or_default().push(path);
    }
    for paths in groups.values_mut() {
        paths.sort();
    }
    groups
}

pub fn runtime_object_lists(manifest: &str) -> Vec<String> {
    manifest
        .lines()
        .map(str::trim)
        .filter(|line| line.ends_with(RUNTIME_OBJECTS_SUFFIX))
        .map(str::to_string)
        .collect()
}

pub fn object_lines(list: &str) -> Vec<String> {
    list.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

pub fn resolve(root: &Path, path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    }
}

/// Where a report the test itself wrote to COVERAGE_OUTPUT_FILE is kept so the merger still sees it.
pub fn preserved_report(output: &Path, coverage_dir: &Path) -> Option<PathBuf> {
    if output.starts_with(coverage_dir) {
        return None;
    }
    Some(coverage_dir.join("test_report.dat"))
}

pub fn export_args(profdata: &Path, root: &Path, objects: &[PathBuf]) -> Vec<String> {
    let mut args = vec![
        "export".to_string(),
        "-format=lcov".to_string(),
        "-instr-profile".to_string(),
        profdata.display().to_string(),
        "-ignore-filename-regex=.*external/.+".to_string(),
        "-ignore-filename-regex=/tmp/.+".to_string(),
        format!("-path-equivalence=.,{}", root.display()),
    ];
    for (i, object) in objects.iter().enumerate() {
        if i > 0 {
            args.push("-object".to_string());
        }
        args.push(object.display().to_string());
    }
    args
}

pub fn rewrite_paths(report: &str, root: &Path) -> String {
    let root = root.display().to_string();
    report
        .replace("/proc/self/cwd/", "")
        .replace(&format!("{root}/"), "")
        .replace(&root, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(magic: u64, version: u64) -> Vec<u8> {
        let mut out = magic.to_le_bytes().to_vec();
        out.extend_from_slice(&version.to_le_bytes());
        out
    }

    #[test]
    fn reads_little_endian_64_bit_header() {
        assert_eq!(raw_profile_version(&header(RAW_MAGIC_64, 10)), Some(10));
    }

    #[test]
    fn reads_32_bit_header() {
        assert_eq!(raw_profile_version(&header(RAW_MAGIC_32, 11)), Some(11));
    }

    #[test]
    fn reads_big_endian_header() {
        let mut out = RAW_MAGIC_64.to_be_bytes().to_vec();
        out.extend_from_slice(&9u64.to_be_bytes());
        assert_eq!(raw_profile_version(&out), Some(9));
    }

    #[test]
    fn drops_variant_flags_from_version() {
        let flagged = (1u64 << 56) | (1u64 << 60) | 10;
        assert_eq!(
            raw_profile_version(&header(RAW_MAGIC_64, flagged)),
            Some(10)
        );
    }

    #[test]
    fn rejects_short_or_foreign_headers() {
        assert_eq!(raw_profile_version(&[0xff, 0x6c]), None);
        assert_eq!(raw_profile_version(&header(0x1234, 10)), None);
    }

    #[test]
    fn groups_files_by_version() {
        let groups = group_by_version(vec![
            (PathBuf::from("b.profraw"), 11),
            (PathBuf::from("a.profraw"), 10),
            (PathBuf::from("c.profraw"), 11),
        ]);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[&10], vec![PathBuf::from("a.profraw")]);
        assert_eq!(
            groups[&11],
            vec![PathBuf::from("b.profraw"), PathBuf::from("c.profraw")]
        );
    }

    #[test]
    fn keeps_only_runtime_object_lists_from_manifest() {
        let manifest = "examples/c-app/cli/cli.c\nbazel-out/k8/bin/examples/c-app/cli/cli_upstreamruntime_objects_list.txt\n\nbazel-out/k8/bin/x.gcno\n";
        assert_eq!(
            runtime_object_lists(manifest),
            vec!["bazel-out/k8/bin/examples/c-app/cli/cli_upstreamruntime_objects_list.txt"]
        );
    }

    #[test]
    fn reads_object_lines() {
        assert_eq!(
            object_lines("bazel-out/k8/bin/a\n\n  bazel-out/k8/bin/b \n"),
            vec!["bazel-out/k8/bin/a", "bazel-out/k8/bin/b"]
        );
    }

    #[test]
    fn keeps_a_report_written_outside_the_coverage_dir() {
        let dir = Path::new("/exec/cov/test");
        assert_eq!(
            preserved_report(Path::new("/exec/testlogs/x/coverage.dat"), dir),
            Some(PathBuf::from("/exec/cov/test/test_report.dat"))
        );
        assert_eq!(
            preserved_report(Path::new("/exec/cov/test/coverage.dat"), dir),
            None
        );
    }

    #[test]
    fn resolves_relative_paths_against_root() {
        let root = Path::new("/exec/root");
        assert_eq!(
            resolve(root, "bazel-out/a"),
            PathBuf::from("/exec/root/bazel-out/a")
        );
        assert_eq!(resolve(root, "/abs/b"), PathBuf::from("/abs/b"));
    }

    #[test]
    fn passes_first_object_positionally() {
        let args = export_args(
            Path::new("p.profdata"),
            Path::new("/r"),
            &[PathBuf::from("a"), PathBuf::from("b"), PathBuf::from("c")],
        );
        assert_eq!(
            args[args.len() - 5..],
            ["a", "-object", "b", "-object", "c"].map(String::from)
        );
        assert!(args.contains(&"-path-equivalence=.,/r".to_string()));
        assert_eq!(
            args[..4],
            ["export", "-format=lcov", "-instr-profile", "p.profdata"].map(String::from)
        );
    }

    #[test]
    fn strips_cwd_and_execroot_prefixes() {
        let report = "SF:/proc/self/cwd/a.c\nSF:/exec/root/b.rs\nSF:c.cc\n";
        assert_eq!(
            rewrite_paths(report, Path::new("/exec/root")),
            "SF:a.c\nSF:b.rs\nSF:c.cc\n"
        );
    }
}
