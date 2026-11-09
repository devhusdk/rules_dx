use std::path::{Path, PathBuf};

pub use crate::collect::rewrite_paths;

pub const PROFDATA_NAME: &str = "Coverage.profdata";

pub fn covered_objects(list: &str) -> Vec<PathBuf> {
    list.split(';')
        .map(str::trim)
        .filter(|object| !object.is_empty())
        .map(PathBuf::from)
        .collect()
}

pub fn llvm_arch(machine: &str) -> &'static str {
    if machine == "aarch64" {
        "arm64"
    } else {
        "x86_64"
    }
}

pub fn apple_export_args(
    profdata: &Path,
    root: &Path,
    objects: &[PathBuf],
    arch: &str,
    manifest: Option<&str>,
) -> Vec<String> {
    let mut args = crate::collect::export_args(profdata, root, objects);
    args.extend(objects.iter().map(|_| format!("-arch={arch}")));
    if let Some(manifest) = manifest {
        args.push(format!("@{manifest}"));
    }
    args
}

pub fn exit_code(test_exit: i32, export_ok: bool) -> i32 {
    if test_exit != 0 {
        test_exit
    } else if export_ok {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_covered_objects_on_semicolons() {
        assert_eq!(
            covered_objects("a/app;b/lib;"),
            vec![PathBuf::from("a/app"), PathBuf::from("b/lib")]
        );
        assert!(covered_objects("").is_empty());
    }

    #[test]
    fn maps_machine_to_llvm_arch() {
        assert_eq!(llvm_arch("aarch64"), "arm64");
        assert_eq!(llvm_arch("x86_64"), "x86_64");
    }

    #[test]
    fn adds_arch_per_object_and_the_manifest() {
        let objects = [PathBuf::from("a"), PathBuf::from("b")];
        let args = apple_export_args(
            Path::new("p"),
            Path::new("/r"),
            &objects,
            "arm64",
            Some("m.txt"),
        );
        assert_eq!(
            args[args.len() - 3..],
            ["-arch=arm64", "-arch=arm64", "@m.txt"].map(String::from)
        );
        assert!(args.contains(&"-instr-profile".to_string()));
    }

    #[test]
    fn omits_the_manifest_when_absent() {
        let args = apple_export_args(
            Path::new("p"),
            Path::new("/r"),
            &[PathBuf::from("a")],
            "x86_64",
            None,
        );
        assert_eq!(args.last().map(String::as_str), Some("-arch=x86_64"));
    }

    #[test]
    fn a_failed_test_always_fails_the_run() {
        assert_eq!(exit_code(3, true), 3);
        assert_eq!(exit_code(3, false), 3);
    }

    #[test]
    fn a_passing_test_follows_the_export() {
        assert_eq!(exit_code(0, true), 0);
        assert_eq!(exit_code(0, false), 1);
    }
}
