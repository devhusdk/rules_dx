//! RubyGems publisher: gem build from staged sources and the gem push.

use crate::run::{
    DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, deploy_build_mode, env_or_empty,
    exit_text, placeholder_refusal, resolve_version, run_publish, scrubbed_command,
};
use crate::stage::*;
use std::collections::BTreeSet;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Owner approval gate for rubygems_deploy.
pub(crate) const RUBYGEMS_GATE: OwnerGate = OwnerGate {
    tool: "rubygems_deploy",
    action: "push",
    credentials: &["RUBYGEMS_API_TOKEN"],
    approved: "RUBYGEMS_PUBLISH_APPROVED",
};

/// Extra env names gem keeps beyond the publish base list.
const GEM_CHILD_ENV_EXTRA: &[&str] = &["GEM_HOME", "GEM_PATH"];

/// Most gem output lines shown when a command fails.
const GEM_FAILURE_LINES: usize = 40;

/// Returns the plain string assigned to `<spec>.<field> = "..."` in a gemspec.
fn gemspec_value(text: &str, field: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        let Some((left, right)) = line.split_once('=') else {
            continue;
        };
        if left.trim_end().rsplit_once('.').map(|(_, last)| last) != Some(field) {
            continue;
        }
        let right = right.trim();
        let quote = right.chars().next().filter(|c| matches!(c, '"' | '\''))?;
        let rest = &right[1..];
        return rest.find(quote).map(|end| rest[..end].to_owned());
    }
    None
}

/// Returns the version to publish, refusing a gemspec that is not the target gem or declares another version.
fn gemspec_version(text: &str, gem_name: &str, version: &str) -> Result<String, String> {
    let name = gemspec_value(text, "name");
    if name.as_deref() != Some(gem_name) {
        return Err(format!(
            "rubygems_deploy: gemspec is {}, target is {gem_name}; refusing",
            name.unwrap_or_else(|| "(not a plain value)".to_owned())
        ));
    }
    resolve_version(
        "rubygems_deploy",
        &format!("{gem_name}.gemspec"),
        gemspec_value(text, "version").as_deref(),
        version,
    )
}

/// Maps gem input files to package-relative paths under the gemspec directory.
pub fn rubygems_package_paths(
    files: &[PathBuf],
    gem_name: &str,
) -> io::Result<Vec<(PathBuf, String)>> {
    package_relative_paths(files, &format!("{gem_name}.gemspec"), "rubygems_deploy")
}

fn rubygems_stage(mapped: &[(PathBuf, String)], staging: &Path) -> Result<(), String> {
    for (src, rel) in mapped {
        let dest = staging.join(rel);
        let staged = dest
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| copy_verified(src, &dest).map(|_| ()));
        if staged.is_err() {
            return Err(format!("rubygems_deploy: cannot stage {}", src.display()));
        }
    }
    Ok(())
}

/// Returns the file paths packed in the data archive of a built gem.
fn gem_file_list(gem: &Path) -> Result<BTreeSet<String>, String> {
    let fail = |error: &dyn std::fmt::Display| {
        format!("rubygems_deploy: cannot read {}: {error}", gem.display())
    };
    let file = std::fs::File::open(gem).map_err(|error| fail(&error))?;
    let mut outer = tar::Archive::new(file);
    for entry in outer.entries().map_err(|error| fail(&error))? {
        let mut entry = entry.map_err(|error| fail(&error))?;
        if entry.path().map_err(|error| fail(&error))?.to_str() != Some("data.tar.gz") {
            continue;
        }
        let mut data = Vec::new();
        entry.read_to_end(&mut data).map_err(|error| fail(&error))?;
        let mut inner = tar::Archive::new(flate2::read::GzDecoder::new(data.as_slice()));
        let mut paths = BTreeSet::new();
        for item in inner.entries().map_err(|error| fail(&error))? {
            let item = item.map_err(|error| fail(&error))?;
            if item.header().entry_type().is_file() {
                let path = item.path().map_err(|error| fail(&error))?;
                paths.insert(path.to_string_lossy().replace('\\', "/"));
            }
        }
        return Ok(paths);
    }
    Err(format!(
        "rubygems_deploy: {} has no data.tar.gz",
        gem.display()
    ))
}

/// Returns a refusal when the gem packs a different file list than the sources.
fn file_list_refusal(
    packed: &BTreeSet<String>,
    mapped: &[(PathBuf, String)],
    gemspec: &str,
) -> Option<String> {
    let want: BTreeSet<String> = mapped
        .iter()
        .map(|(_, rel)| rel.clone())
        .filter(|rel| rel != gemspec)
        .collect();
    if *packed == want {
        return None;
    }
    let missing: Vec<&String> = want.difference(packed).collect();
    let extra: Vec<&String> = packed.difference(&want).collect();
    Some(format!(
        "rubygems_deploy: gem file list differs from the sources; missing {missing:?}, unexpected {extra:?}; refusing"
    ))
}

/// Returns the last output lines with the token value removed.
fn gem_failure_lines(output: &dx_process::capture::Captured, token: &str) -> Vec<String> {
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    if !token.is_empty() {
        text = text.replace(token, "[redacted]");
    }
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(GEM_FAILURE_LINES)..]
        .iter()
        .map(|line| (*line).to_owned())
        .collect()
}

fn gem_failure(what: &str, output: &dx_process::capture::Captured, token: &str) -> String {
    let mut message = format!("rubygems_deploy: {what} failed with {}", exit_text(output));
    for line in gem_failure_lines(output, token) {
        message.push_str("\ngem: ");
        message.push_str(&line);
    }
    message
}

/// Returns PATH with the gem's own directory first, so its `ruby` is the one found.
fn gem_path(program: &Path) -> String {
    let mut dirs: Vec<PathBuf> = program
        .parent()
        .map(Path::to_path_buf)
        .into_iter()
        .collect();
    dirs.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    std::env::join_paths(dirs)
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Runs `gem build` in the staged tree and returns the gem path.
fn gem_build(
    program: &Path,
    staging: &Path,
    gem_name: &str,
    version: &str,
) -> Result<PathBuf, String> {
    let gem = staging.join(format!("{gem_name}-{version}.gem"));
    let path = gem_path(program);
    let output = run_publish(
        scrubbed_command(
            program,
            &[PUBLISH_CHILD_ENV_KEEP, GEM_CHILD_ENV_EXTRA],
            &[("PATH", path.as_str())],
        )
        .current_dir(staging)
        .arg("build")
        .arg(format!("{gem_name}.gemspec"))
        .arg("--output")
        .arg(&gem),
    )
    .map_err(|error| format!("rubygems_deploy: {error}"))?;
    if !output.success() {
        return Err(gem_failure("gem build", &output, ""));
    }
    Ok(gem)
}

/// Runs `gem push`; the key goes through GEM_HOST_API_KEY.
fn gem_push(program: &Path, gem: &Path, token: &str) -> Result<(), String> {
    let path = gem_path(program);
    let output = run_publish(
        scrubbed_command(
            program,
            &[PUBLISH_CHILD_ENV_KEEP, GEM_CHILD_ENV_EXTRA],
            &[("GEM_HOST_API_KEY", token), ("PATH", path.as_str())],
        )
        .arg("push")
        .arg(gem),
    )
    .map_err(|error| format!("rubygems_deploy: {error}"))?;
    if output.success() {
        return Ok(());
    }
    Err(gem_failure("gem push", &output, token))
}

/// Stages the sources, checks the gemspec, builds the gem and checks its file list; returns the gem and its version.
fn rubygems_build_checked(
    gem_program: &Path,
    mapped: &[(PathBuf, String)],
    staging: &Path,
    gem_name: &str,
    version: &str,
) -> Result<(PathBuf, String), String> {
    rubygems_stage(mapped, staging)?;
    let gemspec = format!("{gem_name}.gemspec");
    let text = std::fs::read_to_string(staging.join(&gemspec))
        .map_err(|error| format!("rubygems_deploy: cannot read {gemspec}: {error}"))?;
    let version = gemspec_version(&text, gem_name, version)?;
    let gem = gem_build(gem_program, staging, gem_name, &version)?;
    if let Some(error) = file_list_refusal(&gem_file_list(&gem)?, mapped, &gemspec) {
        return Err(error);
    }
    Ok((gem, version))
}

/// Finds `gem` next to the pinned `ruby`, or on PATH when none is pinned.
fn find_gem(run: &DeployRun, ruby_rloc: &str) -> Option<PathBuf> {
    if ruby_rloc.is_empty() {
        let gem = which_on_path("gem");
        if gem.is_none() {
            eprintln!("rubygems_deploy: gem not found on PATH; install Ruby or set ruby");
        }
        return gem;
    }
    let ruby = run.resolve(ruby_rloc)?;
    let name = if cfg!(windows) { "gem.cmd" } else { "gem" };
    let gem = ruby
        .parent()
        .map(|bin| bin.join(name))
        .filter(|gem| gem.is_file());
    if gem.is_none() {
        eprintln!("rubygems_deploy: no {name} next to {}", ruby.display());
    }
    gem
}

/// Runs the RubyGems deploy tool.
pub fn rubygems_main(
    gem_rlocs: &[String],
    gem_name: &str,
    version: &str,
    ruby_rloc: &str,
    argv: &[String],
) -> i32 {
    let Some(run) = DeployRun::start(
        "rubygems_deploy",
        "the gem is exactly the files pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let live = RUBYGEMS_GATE.requested(env_or_empty);
    let secrets = if live {
        match RUBYGEMS_GATE.authorize(env_or_empty) {
            Ok(secrets) => Some(secrets),
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        }
    } else {
        None
    };
    let mut files = Vec::new();
    for rloc in gem_rlocs.iter().filter(|s| !s.is_empty()) {
        let Some(path) = run.resolve(rloc) else {
            return 1;
        };
        files.push(path);
    }
    let Some(gem_program) = find_gem(&run, ruby_rloc) else {
        return 1;
    };
    let staging = match tempfile_stage_dir("rubygems-") {
        Ok(dir) => dir,
        Err(error) => {
            eprintln!("rubygems_deploy: {error}");
            return 1;
        }
    };
    let built = rubygems_package_paths(&files, gem_name)
        .map_err(|error| format!("rubygems_deploy: {error}"))
        .and_then(|mapped| {
            rubygems_build_checked(&gem_program, &mapped, &staging, gem_name, version)
        });
    let result = built.and_then(|(gem, version)| match &secrets {
        Some(secrets) => {
            if let Some(error) = placeholder_refusal("rubygems_deploy", &version) {
                return Err(error);
            }
            gem_push(&gem_program, &gem, secrets.get("RUBYGEMS_API_TOKEN"))
                .map(|()| format!("rubygems_deploy: pushed {gem_name} {version}"))
        }
        None => {
            let dest = run.outdir().join(format!("{gem_name}-{version}.gem"));
            copy_verified(&gem, &dest).map_err(|error| format!("rubygems_deploy: {error}"))?;
            let build_mode = deploy_build_mode();
            Ok(format!(
                "rubygems_deploy: staged {gem_name} {version} (build_mode: {build_mode})\n  gem: {}",
                dest.display()
            ))
        }
    });
    let _ = std::fs::remove_dir_all(&staging);
    match result {
        Ok(message) => {
            println!("{message}");
            0
        }
        Err(message) => {
            eprintln!("{message}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    const GEMSPEC: &str = "Gem::Specification.new do |spec|\n  spec.name = \"demo\"\n  spec.version = '1.2.3'\n  spec.files = [\"lib/demo.rb\"]\nend\n";

    #[test]
    fn gemspec_must_match_target() {
        assert_eq!(
            gemspec_version(GEMSPEC, "demo", "1.2.3"),
            Ok("1.2.3".to_owned())
        );
        assert_eq!(gemspec_version(GEMSPEC, "demo", ""), Ok("1.2.3".to_owned()));
        let wrong = gemspec_version(GEMSPEC, "other", "").expect_err("name refused");
        assert!(
            wrong.contains("gemspec is demo, target is other"),
            "{wrong}"
        );
        let mismatch = gemspec_version(GEMSPEC, "demo", "2.0.0").expect_err("version refused");
        assert!(
            mismatch.contains("declares version 1.2.3, BUILD declares 2.0.0"),
            "{mismatch}"
        );
        assert!(gemspec_version("spec.version = Demo::VERSION\n", "demo", "").is_err());
    }

    #[test]
    fn file_list_must_match_sources() {
        let mapped = vec![
            (PathBuf::from("a"), "demo.gemspec".to_owned()),
            (PathBuf::from("b"), "lib/demo.rb".to_owned()),
        ];
        let packed: BTreeSet<String> = ["lib/demo.rb".to_owned()].into();
        assert_eq!(file_list_refusal(&packed, &mapped, "demo.gemspec"), None);
        let extra: BTreeSet<String> = ["lib/demo.rb".to_owned(), "x.rb".to_owned()].into();
        let message = file_list_refusal(&extra, &mapped, "demo.gemspec").expect("refused");
        assert!(message.contains("unexpected [\"x.rb\"]"), "{message}");
        let empty = BTreeSet::new();
        assert!(file_list_refusal(&empty, &mapped, "demo.gemspec").is_some());
    }

    fn write_gem(path: &Path, files: &[&str]) {
        let mut inner = tar::Builder::new(Vec::new());
        for name in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(2);
            header.set_mode(0o644);
            inner
                .append_data(&mut header, name, &b"x\n"[..])
                .expect("inner");
        }
        let mut data = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        io::Write::write_all(&mut data, &inner.into_inner().expect("tar")).expect("gzip");
        let data = data.finish().expect("finish");
        let mut outer = tar::Builder::new(std::fs::File::create(path).expect("gem"));
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        outer
            .append_data(&mut header, "data.tar.gz", data.as_slice())
            .expect("outer");
        outer.finish().expect("finish");
    }

    #[test]
    fn gem_file_list_reads_data_archive() {
        let scratch = scratch_dir();
        let gem = scratch.path().join("demo-1.2.3.gem");
        write_gem(&gem, &["lib/demo.rb", "lib/demo/util.rb"]);
        let want: BTreeSet<String> = ["lib/demo.rb", "lib/demo/util.rb"]
            .map(str::to_owned)
            .into();
        assert_eq!(gem_file_list(&gem), Ok(want));
        std::fs::write(&gem, b"not a tar").expect("corrupt");
        assert!(gem_file_list(&gem).is_err());
    }

    #[test]
    fn package_paths_root_at_the_gemspec() {
        let scratch = scratch_dir();
        let root = scratch.path().join("in/pkg");
        let files = vec![
            write_crate_file(&root, "demo.gemspec", GEMSPEC),
            write_crate_file(&root, "lib/demo.rb", ""),
        ];
        let mapped = rubygems_package_paths(&files, "demo").expect("paths");
        let rels: Vec<&str> = mapped.iter().map(|(_, rel)| rel.as_str()).collect();
        assert_eq!(rels, ["demo.gemspec", "lib/demo.rb"]);
        assert!(rubygems_package_paths(&files, "other").is_err());
    }

    #[cfg(unix)]
    fn fake_gem(dir: &Path, body: &str) -> PathBuf {
        let fake = dir.join("gem");
        dx_testing::executable_script(&fake, &format!("#!/bin/sh\n{body}\n"));
        fake
    }

    #[cfg(unix)]
    #[test]
    fn build_and_push_use_gem_and_hide_the_token() {
        let scratch = scratch_dir();
        let root = scratch.path().join("in/pkg");
        let files = vec![
            write_crate_file(&root, "demo.gemspec", GEMSPEC),
            write_crate_file(&root, "lib/demo.rb", ""),
        ];
        let mapped = rubygems_package_paths(&files, "demo").expect("paths");
        let prebuilt = scratch.path().join("prebuilt.gem");
        write_gem(&prebuilt, &["lib/demo.rb"]);
        let bin = scratch.path().join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        let build = fake_gem(
            &bin,
            &format!("[ \"$1\" = build ] && cp {} \"$4\"", prebuilt.display()),
        );
        let staging = scratch.path().join("staging");
        let (gem, version) =
            rubygems_build_checked(&build, &mapped, &staging, "demo", "").expect("build");
        assert_eq!(version, "1.2.3");
        assert_eq!(
            gem.file_name().and_then(|n| n.to_str()),
            Some("demo-1.2.3.gem")
        );

        let wrong = scratch.path().join("wrong.gem");
        write_gem(&wrong, &["lib/demo.rb", "secret.rb"]);
        let bad = fake_gem(
            &bin,
            &format!("[ \"$1\" = build ] && cp {} \"$4\"", wrong.display()),
        );
        let error =
            rubygems_build_checked(&bad, &mapped, &scratch.path().join("s2"), "demo", "1.2.3")
                .expect_err("file list refused");
        assert!(error.contains("unexpected [\"secret.rb\"]"), "{error}");

        let push = fake_gem(
            &bin,
            "echo \"Repushing of gem versions is not allowed $GEM_HOST_API_KEY\" >&2\nexit 1",
        );
        let message = gem_push(&push, &gem, "s3cret").expect_err("push fails");
        assert!(
            message.contains("gem push failed with exit status: 1"),
            "{message}"
        );
        assert!(
            message.contains("\ngem: Repushing of gem versions is not allowed [redacted]"),
            "{message}"
        );
        assert!(!message.contains("s3cret"), "{message}");
        let ok = fake_gem(&bin, "[ \"$GEM_HOST_API_KEY\" = s3cret ]");
        assert!(gem_push(&ok, &gem, "s3cret").is_ok());
    }
}
