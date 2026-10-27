//! crates.io publisher: vendor staging and the cargo publish.

use crate::run::{deploy_profile, env_or_empty, scrubbed_command, DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP};
use crate::stage::*;
use std::io;
use std::path::Path;

/// Maps crate input files to package-relative paths under the Cargo.toml directory.
pub fn crates_package_paths(
    crate_files: &[std::path::PathBuf],
) -> io::Result<Vec<(std::path::PathBuf, String)>> {
    if crate_files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "crates vendor: need at least one crate source file",
        ));
    }
    package_relative_paths(crate_files, "Cargo.toml", "crates vendor")
}

/// Builds one crates vendor tree.
pub fn crates_build_vendor(
    crate_files: &[std::path::PathBuf],
    outdir: &Path,
    crate_name: &str,
    version: &str,
) -> io::Result<std::path::PathBuf> {
    let mapped = crates_package_paths(crate_files)?;
    let house = outdir.join(crate_name.to_owned() + "-vendor");
    let vendor_dir = house.join("vendor").join(crate_name);
    let registry_dir = house.join("registry").join(crate_name).join(version);
    std::fs::create_dir_all(&vendor_dir)?;
    std::fs::create_dir_all(&registry_dir)?;
    let mut entries: Vec<(String, String)> = Vec::new();
    for (src, base) in &mapped {
        let want = dx_digest::sha256_file_hex(src)?;
        for dest_dir in [&vendor_dir, &registry_dir] {
            let dest = dest_dir.join(base);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            dx_atomic_fs::copy_atomic(src, &dest)?;
            let got = dx_digest::sha256_file_hex(&dest)?;
            if want != got {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "crates vendor: byte mismatch for {base} (expected sha256 {want}, got {got})"
                    ),
                ));
            }
        }
        entries.push((base.clone(), want));
    }
    entries.sort();
    let package_digest = dx_digest::sha256_hex(
        entries
            .iter()
            .map(|(base, digest)| format!("{base}:{digest}"))
            .collect::<Vec<_>>()
            .join("\n")
            .as_bytes(),
    );
    let mut files = serde_json::Map::new();
    for (base, digest) in &entries {
        files.insert(base.clone(), serde_json::Value::String(digest.clone()));
    }
    let checksum_body = render_json(&serde_json::json!({
        "files": serde_json::Value::Object(files),
        "package": package_digest,
    }))?;
    std::fs::write(
        vendor_dir.join(".cargo-checksum.json"),
        checksum_body.as_bytes(),
    )?;
    let index_body = render_json(&serde_json::json!({
        "files": entries
            .iter()
            .map(|(base, digest)| serde_json::json!({"name": base, "sha256": digest}))
            .collect::<Vec<_>>(),
        "name": crate_name,
        "version": version,
    }))?;
    std::fs::write(registry_dir.join("index.json"), index_body.as_bytes())?;
    Ok(house)
}

/// Owner approval gate for crates_deploy.
pub(crate) const CRATES_GATE: OwnerGate = OwnerGate {
    tool: "crates_deploy",
    action: "publish",
    credentials: &["CARGO_REGISTRY_TOKEN"],
    approved: "CRATES_PUBLISH_APPROVED",
    live: "CRATES_PUBLISH_LIVE",
};

/// Extra env names cargo keeps beyond the publish base list.
const CARGO_CHILD_ENV_EXTRA: &[&str] = &["CARGO_HOME", "RUSTUP_HOME"];

/// Runs the crates deploy launcher.
pub fn crates_main(crate_rlocs: &str, crate_name: &str, version: &str, argv: &[String]) -> i32 {
    let Some(run) = DeployRun::start(
        "crates_deploy",
        "the vendor tree is exactly the files pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let outdir = run.outdir();
    let mut files = Vec::new();
    for rloc in crate_rlocs.split(';').filter(|s| !s.is_empty()) {
        let Some(path) = run.resolve(rloc) else {
            return 1;
        };
        files.push(path);
    }
    if CRATES_GATE.requested(env_or_empty) {
        if std::env::var("CRATES_ALLOW_DIRTY").unwrap_or_default() == "1" {
            eprintln!(
                "crates_deploy: --allow-dirty rejected by default; publish from a clean tree instead"
            );
            return 1;
        }
        if version == "0.0.0" {
            eprintln!("crates_deploy: live publish refuses version 0.0.0; set a real version");
            return 1;
        }
        let secrets = match CRATES_GATE.authorize(env_or_empty) {
            Ok(secrets) => secrets,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        let staging = match tempfile_stage_dir("crates-publish-") {
            Ok(dir) => dir,
            Err(error) => {
                eprintln!("crates_deploy: {error}");
                return 1;
            }
        };
        let mapped = match crates_package_paths(&files) {
            Ok(mapped) => mapped,
            Err(error) => {
                eprintln!("crates_deploy: {error}");
                return 1;
            }
        };
        for (src, rel) in &mapped {
            let dest = staging.join(rel);
            let staged = dest
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::copy(src, &dest).map(|_| ()));
            if staged.is_err() {
                eprintln!("crates_deploy: cannot stage {}", src.display());
                return 1;
            }
        }
        let manifest = staging.join("Cargo.toml");
        if !manifest.is_file() {
            eprintln!("crates_deploy: staged crate is missing Cargo.toml; refusing");
            return 1;
        }
        let mut cmd = scrubbed_command(
            Path::new("cargo"),
            &[PUBLISH_CHILD_ENV_KEEP, CARGO_CHILD_ENV_EXTRA],
            &[("CARGO_REGISTRY_TOKEN", secrets.get("CARGO_REGISTRY_TOKEN"))],
        );
        cmd.arg("publish").arg("--manifest-path").arg(&manifest);
        let output = cmd.output();
        let _ = std::fs::remove_dir_all(&staging);
        match output {
            Ok(output) if output.status.success() => {
                println!("crates_deploy: published {crate_name} {version}");
                return 0;
            }
            Ok(output) => {
                eprintln!("crates_deploy: cargo publish failed with {}", output.status);
                return 1;
            }
            Err(error) => {
                eprintln!("crates_deploy: {error}");
                return 1;
            }
        }
    }
    match crates_build_vendor(&files, outdir, crate_name, version) {
        Ok(house) => {
            let profile = deploy_profile();
            println!("crates_deploy: staged {crate_name} {version} (profile: {profile})");
            println!(
                "  vendor: {}",
                house.join("vendor").join(crate_name).display()
            );
            println!(
                "  registry: {}",
                house
                    .join("registry")
                    .join(crate_name)
                    .join(version)
                    .display()
            );
            0
        }
        Err(error) => {
            eprintln!("crates_deploy: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::basename;
    use crate::test_support::*;

    #[test]
    fn crates_vendor_builds_checksums() {
        let scratch = scratch_dir();
        let manifest = scratch.path().join("Cargo.toml");
        std::fs::write(
            &manifest,
            b"[package]\nname = \"crates_demo\"\nversion = \"0.0.0\"\n",
        )
        .expect("write manifest");
        let source = scratch.path().join("lib.rs");
        std::fs::write(&source, b"pub fn hello() -> &str { \"hello\" }\n").expect("write src");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let files = vec![manifest.clone(), source.clone()];
        let house = crates_build_vendor(&files, &outdir, "crates_demo", "0.0.0").expect("vendor");
        let checksum = std::fs::read_to_string(
            house
                .join("vendor")
                .join("crates_demo")
                .join(".cargo-checksum.json"),
        )
        .expect("checksum");
        let pairs = files
            .iter()
            .map(|src| {
                (
                    basename(src).expect("base"),
                    dx_digest::sha256_file_hex(src).expect("hex"),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        assert!(checksum.ends_with('\n'));
        let parsed: serde_json::Value = serde_json::from_str(&checksum).expect("checksum parses");
        let want_files = serde_json::json!({
            "Cargo.toml": pairs["Cargo.toml"],
            "lib.rs": pairs["lib.rs"],
        });
        assert_eq!(parsed["files"], want_files);
        let package = parsed["package"].as_str().expect("package");
        assert_eq!(package.len(), 64);
        assert!(package.bytes().all(|b| b.is_ascii_hexdigit()));
        let index = std::fs::read_to_string(
            house
                .join("registry")
                .join("crates_demo")
                .join("0.0.0")
                .join("index.json"),
        )
        .expect("index");
        assert!(index.ends_with('\n'));
        let index: serde_json::Value = serde_json::from_str(&index).expect("index parses");
        assert_eq!(index["name"], "crates_demo");
        assert_eq!(index["version"], "0.0.0");
        assert_eq!(index["files"][0]["name"], "Cargo.toml");
        assert_eq!(index["files"][0]["sha256"], pairs["Cargo.toml"].as_str());
        assert!(crates_build_vendor(&[], &outdir, "crates_demo", "0.0.0").is_err());
    }

    fn nested_crate(root: &Path) -> Vec<std::path::PathBuf> {
        vec![
            write_crate_file(
                root,
                "Cargo.toml",
                "[package]\nname = \"crates_demo\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
            ),
            write_crate_file(root, "src/lib.rs", "pub mod parser;\npub mod lexer;\n"),
            write_crate_file(root, "src/parser/mod.rs", "pub fn parse() -> u8 { 1 }\n"),
            write_crate_file(root, "src/lexer/mod.rs", "pub fn lex() -> u8 { 2 }\n"),
        ]
    }

    #[test]
    fn crates_vendor_preserves_package_paths() {
        let scratch = scratch_dir();
        let files = nested_crate(&scratch.path().join("in/pkg"));
        let outdir = scratch.path().join("out");
        let house = crates_build_vendor(&files, &outdir, "crates_demo", "0.0.0").expect("vendor");
        for base in [
            house.join("vendor/crates_demo"),
            house.join("registry/crates_demo/0.0.0"),
        ] {
            for rel in [
                "Cargo.toml",
                "src/lib.rs",
                "src/parser/mod.rs",
                "src/lexer/mod.rs",
            ] {
                assert!(base.join(rel).is_file(), "{}", base.join(rel).display());
            }
        }
        let checksum =
            std::fs::read_to_string(house.join("vendor/crates_demo/.cargo-checksum.json"))
                .expect("checksum");
        assert!(checksum.contains("\"src/parser/mod.rs\""), "{checksum}");
        assert!(checksum.contains("\"src/lexer/mod.rs\""), "{checksum}");
    }

    #[test]
    fn crates_vendor_keeps_explicit_manifest_paths() {
        let scratch = scratch_dir();
        let root = scratch.path().join("in");
        let files = vec![
            write_crate_file(
                &root,
                "Cargo.toml",
                "[package]\nname = \"crates_demo\"\nversion = \"0.0.0\"\n[lib]\npath = \"custom/entry.rs\"\n",
            ),
            write_crate_file(&root, "custom/entry.rs", "pub fn hello() {}\n"),
        ];
        let house =
            crates_build_vendor(&files, &scratch.path().join("out"), "crates_demo", "0.0.0")
                .expect("vendor");
        assert!(house.join("vendor/crates_demo/custom/entry.rs").is_file());
    }

    #[test]
    fn crates_package_paths_reject_collisions_and_escapes() {
        let scratch = scratch_dir();
        let root = scratch.path().join("in");
        let manifest = write_crate_file(&root, "Cargo.toml", "[package]\n");
        let outside = write_crate_file(scratch.path(), "other/lib.rs", "");
        let error = crates_package_paths(&[manifest.clone(), outside]).expect_err("outside");
        assert!(
            error.to_string().contains("outside the package root"),
            "{error}"
        );
        let dotdot = root.join("src/../lib.rs");
        write_crate_file(&root, "lib.rs", "");
        let error = crates_package_paths(&[manifest.clone(), dotdot]).expect_err("dotdot");
        assert!(error.to_string().contains("unsafe package path"), "{error}");
        let error =
            crates_package_paths(&[manifest.clone(), root.join("lib.rs"), root.join("lib.rs")])
                .expect_err("dup");
        assert!(error.to_string().contains("path collision"), "{error}");
        let file = write_crate_file(&root, "src", "");
        let nested = write_crate_file(&root, "src_dir/x.rs", "");
        assert!(crates_package_paths(&[manifest.clone(), file, nested]).is_ok());
        let file = root.join("src");
        let child = root.join("src/lib.rs");
        let error = crates_package_paths(&[manifest.clone(), file, child]).expect_err("file+dir");
        assert!(
            error.to_string().contains("both a file and a directory"),
            "{error}"
        );
        let error = crates_package_paths(&[write_crate_file(&root, "lib.rs", "")])
            .expect_err("no manifest");
        assert!(error.to_string().contains("no Cargo.toml"), "{error}");
        let error = crates_package_paths(&[
            write_crate_file(&root, "a/Cargo.toml", ""),
            write_crate_file(&root, "b/Cargo.toml", ""),
        ])
        .expect_err("ambiguous");
        assert!(
            error.to_string().contains("ambiguous package root"),
            "{error}"
        );
    }

    #[test]
    fn crates_vendor_builds_with_cargo_consumer() {
        let Ok(probe) = std::process::Command::new("cargo")
            .arg("--version")
            .output()
        else {
            return;
        };
        if !probe.status.success() {
            return;
        }
        let scratch = scratch_dir();
        let files = nested_crate(&scratch.path().join("in/pkg"));
        let house =
            crates_build_vendor(&files, &scratch.path().join("out"), "crates_demo", "0.0.0")
                .expect("vendor");
        let consumer = scratch.path().join("consumer");
        write_crate_file(
            &consumer,
            "Cargo.toml",
            "[package]\nname = \"consumer\"\nversion = \"0.0.0\"\nedition = \"2021\"\n[dependencies]\ncrates_demo = \"0.0.0\"\n",
        );
        write_crate_file(
            &consumer,
            "src/main.rs",
            "fn main() { println!(\"{}\", crates_demo::parser::parse() + crates_demo::lexer::lex()); }\n",
        );
        write_crate_file(
            &consumer,
            ".cargo/config.toml",
            &format!(
                "[source.crates-io]\nreplace-with = \"vendored\"\n[source.vendored]\ndirectory = \"{}\"\n",
                house.join("vendor").display().to_string().replace('\\', "/")
            ),
        );
        let output = std::process::Command::new("cargo")
            .args(["run", "--offline", "--quiet"])
            .current_dir(&consumer)
            .env("CARGO_TARGET_DIR", scratch.path().join("target"))
            .env("CARGO_HOME", scratch.path().join("cargo_home"))
            .output()
            .expect("cargo run");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "3");
    }
}
