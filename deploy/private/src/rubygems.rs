use crate::run::{
    DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, deploy_build_mode, env_or_empty, exit_text,
    placeholder_refusal, resolve_version, run_publish, scrubbed_command,
};
use crate::stage::*;
use dx_atomic_fs::path_io::{self, PathIoError};
use dx_process::capture::RunError;
use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum RubygemsError {
    Paths(io::Error),
    Stage(StageError),
    TempDir(io::Error),
    Io(PathIoError),
    Run(RunError),
    NameMismatch {
        found: Option<String>,
        target: String,
    },
    Refused(String),
    NoDataArchive {
        gem: PathBuf,
    },
    FileListMismatch {
        missing: Vec<String>,
        extra: Vec<String>,
    },
    GemFailed {
        step: &'static str,
        status: String,
        lines: Vec<String>,
    },
    Copy(io::Error),
}

impl fmt::Display for RubygemsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Paths(source) | Self::TempDir(source) | Self::Copy(source) => {
                write!(f, "rubygems_deploy: {source}")
            }
            Self::Stage(source) => write!(f, "rubygems_deploy: {source}"),
            Self::Io(source) => write!(f, "rubygems_deploy: {source}"),
            Self::Run(source) => write!(f, "rubygems_deploy: {source}"),
            Self::NameMismatch { found, target } => write!(
                f,
                "rubygems_deploy: gemspec is {}, target is {target}; refusing",
                found.as_deref().unwrap_or("(not a plain value)")
            ),
            Self::Refused(message) => f.write_str(message),
            Self::NoDataArchive { gem } => {
                write!(f, "rubygems_deploy: {} has no data.tar.gz", gem.display())
            }
            Self::FileListMismatch { missing, extra } => write!(
                f,
                "rubygems_deploy: gem file list differs from the sources; missing {missing:?}, unexpected {extra:?}; refusing"
            ),
            Self::GemFailed {
                step,
                status,
                lines,
            } => {
                write!(f, "rubygems_deploy: {step} failed with {status}")?;
                for line in lines {
                    write!(f, "\ngem: {line}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for RubygemsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Paths(source) | Self::TempDir(source) | Self::Copy(source) => Some(source),
            Self::Stage(source) => Some(source),
            Self::Io(source) => Some(source),
            Self::Run(source) => Some(source),
            Self::NameMismatch { .. }
            | Self::Refused(_)
            | Self::NoDataArchive { .. }
            | Self::FileListMismatch { .. }
            | Self::GemFailed { .. } => None,
        }
    }
}

pub(crate) const RUBYGEMS_GATE: OwnerGate = OwnerGate {
    tool: "rubygems_deploy",
    action: "push",
    credentials: &["RUBYGEMS_API_TOKEN"],
    approved: "RUBYGEMS_PUBLISH_APPROVED",
};

const GEM_CHILD_ENV_EXTRA: &[&str] = &["GEM_HOME", "GEM_PATH"];

const GEM_FAILURE_LINES: usize = 40;

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
        let mut chars = right.chars();
        let quote = chars.next().filter(|c| matches!(c, '"' | '\''))?;
        return chars
            .as_str()
            .split_once(quote)
            .map(|(value, _)| value.to_owned());
    }
    None
}

fn gemspec_version(text: &str, gem_name: &str, version: &str) -> Result<String, RubygemsError> {
    let name = gemspec_value(text, "name");
    if name.as_deref() != Some(gem_name) {
        return Err(RubygemsError::NameMismatch {
            found: name,
            target: gem_name.to_owned(),
        });
    }
    resolve_version(
        "rubygems_deploy",
        &format!("{gem_name}.gemspec"),
        gemspec_value(text, "version").as_deref(),
        version,
    )
    .map_err(RubygemsError::Refused)
}

pub fn rubygems_package_paths(
    files: &[PathBuf],
    gem_name: &str,
) -> io::Result<Vec<(PathBuf, String)>> {
    package_relative_paths(files, &format!("{gem_name}.gemspec"), "rubygems_deploy")
}

fn gem_file_list(gem: &Path) -> Result<BTreeSet<String>, RubygemsError> {
    let fail = |source: io::Error| {
        RubygemsError::Io(PathIoError::Read {
            path: gem.to_path_buf(),
            source,
        })
    };
    let file = std::fs::File::open(gem).map_err(fail)?;
    let mut data = None;
    dx_archive::visit(
        file,
        dx_archive::Format::Tar,
        crate::archive::LISTING_LIMITS,
        |name, _, body| {
            if data.is_none() && name == "data.tar.gz" {
                let mut bytes = Vec::new();
                body.read_to_end(&mut bytes)?;
                data = Some(bytes);
            }
            Ok(())
        },
    )
    .map_err(fail)?;
    if let Some(data) = data {
        let mut paths = BTreeSet::new();
        dx_archive::visit(
            io::Cursor::new(data),
            dx_archive::Format::TarGz,
            crate::archive::LISTING_LIMITS,
            |name, kind, _| {
                if kind == dx_archive::EntryKind::File {
                    paths.insert(name.to_owned());
                }
                Ok(())
            },
        )
        .map_err(fail)?;
        return Ok(paths);
    }
    Err(RubygemsError::NoDataArchive {
        gem: gem.to_path_buf(),
    })
}

fn file_list_refusal(
    packed: &BTreeSet<String>,
    mapped: &[(PathBuf, String)],
    gemspec: &str,
) -> Option<RubygemsError> {
    let want: BTreeSet<String> = mapped
        .iter()
        .map(|(_, rel)| rel.clone())
        .filter(|rel| rel != gemspec)
        .collect();
    if *packed == want {
        return None;
    }
    Some(RubygemsError::FileListMismatch {
        missing: want.difference(packed).cloned().collect(),
        extra: packed.difference(&want).cloned().collect(),
    })
}

fn gem_failure_lines(output: &dx_process::capture::Captured, token: &str) -> Vec<String> {
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    if !token.is_empty() {
        text = text.replace(token, "[redacted]");
    }
    let lines: Vec<&str> = text.lines().collect();
    lines
        .iter()
        .skip(lines.len().saturating_sub(GEM_FAILURE_LINES))
        .map(|line| (*line).to_owned())
        .collect()
}

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

fn gem_build(
    program: &Path,
    staging: &Path,
    gem_name: &str,
    version: &str,
) -> Result<PathBuf, RubygemsError> {
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
    .map_err(RubygemsError::Run)?;
    if !output.success() {
        return Err(RubygemsError::GemFailed {
            step: "gem build",
            status: exit_text(&output),
            lines: gem_failure_lines(&output, ""),
        });
    }
    Ok(gem)
}

fn gem_push(program: &Path, gem: &Path, token: &str) -> Result<(), RubygemsError> {
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
    .map_err(RubygemsError::Run)?;
    if output.success() {
        return Ok(());
    }
    Err(RubygemsError::GemFailed {
        step: "gem push",
        status: exit_text(&output),
        lines: gem_failure_lines(&output, token),
    })
}

fn rubygems_build_checked(
    gem_program: &Path,
    mapped: &[(PathBuf, String)],
    staging: &Path,
    gem_name: &str,
    version: &str,
) -> Result<(PathBuf, String), RubygemsError> {
    stage_mapped(mapped, staging).map_err(RubygemsError::Stage)?;
    let gemspec = format!("{gem_name}.gemspec");
    let text = path_io::read_to_string(&staging.join(&gemspec)).map_err(RubygemsError::Io)?;
    let version = gemspec_version(&text, gem_name, version)?;
    let gem = gem_build(gem_program, staging, gem_name, &version)?;
    if let Some(error) = file_list_refusal(&gem_file_list(&gem)?, mapped, &gemspec) {
        return Err(error);
    }
    Ok((gem, version))
}

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
    let live = OwnerGate::requested(env_or_empty);
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
            eprintln!("{}", RubygemsError::TempDir(error));
            return 1;
        }
    };
    let built = rubygems_package_paths(&files, gem_name)
        .map_err(RubygemsError::Paths)
        .and_then(|mapped| {
            rubygems_build_checked(&gem_program, &mapped, &staging, gem_name, version)
        });
    let result = built.and_then(|(gem, version)| {
        if let Some(secrets) = &secrets {
            if let Some(error) = placeholder_refusal("rubygems_deploy", &version) {
                return Err(RubygemsError::Refused(error));
            }
            gem_push(&gem_program, &gem, secrets.get("RUBYGEMS_API_TOKEN"))
                .map(|()| format!("rubygems_deploy: pushed {gem_name} {version}"))
        } else {
            let dest = run.outdir().join(format!("{gem_name}-{version}.gem"));
            copy_verified(&gem, &dest).map_err(RubygemsError::Copy)?;
            let build_mode = deploy_build_mode();
            Ok(format!(
                "rubygems_deploy: staged {gem_name} {version} (build_mode: {build_mode})\n  gem: {}",
                dest.display()
            ))
        }
    });
    dx_atomic_fs::discard_dir(&staging);
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
            gemspec_version(GEMSPEC, "demo", "1.2.3").expect("matching version"),
            "1.2.3"
        );
        assert_eq!(
            gemspec_version(GEMSPEC, "demo", "").expect("declared version"),
            "1.2.3"
        );
        let wrong = gemspec_version(GEMSPEC, "other", "").expect_err("name refused");
        assert!(
            matches!(wrong, RubygemsError::NameMismatch { .. }),
            "{wrong:?}"
        );
        assert!(
            wrong
                .to_string()
                .contains("gemspec is demo, target is other"),
            "{wrong}"
        );
        let mismatch = gemspec_version(GEMSPEC, "demo", "2.0.0").expect_err("version refused");
        assert!(
            matches!(mismatch, RubygemsError::Refused(_)),
            "{mismatch:?}"
        );
        assert!(
            mismatch
                .to_string()
                .contains("declares version 1.2.3, BUILD declares 2.0.0"),
            "{mismatch}"
        );
        let unnamed =
            gemspec_version("spec.version = Demo::VERSION\n", "demo", "").expect_err("no name");
        assert!(
            unnamed
                .to_string()
                .contains("gemspec is (not a plain value), target is demo"),
            "{unnamed}"
        );
    }

    #[test]
    fn file_list_must_match_sources() {
        let mapped = vec![
            (PathBuf::from("a"), "demo.gemspec".to_owned()),
            (PathBuf::from("b"), "lib/demo.rb".to_owned()),
        ];
        let packed: BTreeSet<String> = ["lib/demo.rb".to_owned()].into();
        assert!(file_list_refusal(&packed, &mapped, "demo.gemspec").is_none());
        let extra: BTreeSet<String> = ["lib/demo.rb".to_owned(), "x.rb".to_owned()].into();
        let message = file_list_refusal(&extra, &mapped, "demo.gemspec")
            .expect("refused")
            .to_string();
        assert!(
            message.contains("missing [], unexpected [\"x.rb\"]"),
            "{message}"
        );
        let empty = BTreeSet::new();
        assert!(file_list_refusal(&empty, &mapped, "demo.gemspec").is_some());
    }

    fn write_gem(path: &Path, files: &[&str]) {
        let mut inner = dx_archive::TarWriter::new();
        for name in files {
            inner.add_file(name, b"x\n", 0o644).expect("inner");
        }
        let data = inner.finish().expect("data.tar.gz");
        let mut outer = dx_archive::TarWriter::new();
        outer.add_file("data.tar.gz", &data, 0o644).expect("outer");
        std::fs::write(path, outer.finish_tar().expect("finish")).expect("gem");
    }

    #[test]
    fn gem_file_list_reads_data_archive() {
        let scratch = scratch_dir();
        let gem = scratch.path().join("demo-1.2.3.gem");
        write_gem(&gem, &["lib/demo.rb", "lib/demo/util.rb"]);
        let want: BTreeSet<String> = ["lib/demo.rb", "lib/demo/util.rb"]
            .map(str::to_owned)
            .into();
        assert_eq!(gem_file_list(&gem).expect("file list"), want);
        std::fs::write(&gem, b"not a tar").expect("corrupt");
        let error = gem_file_list(&gem).expect_err("corrupt gem");
        assert!(
            error
                .to_string()
                .starts_with(&format!("rubygems_deploy: cannot read {}: ", gem.display())),
            "{error}"
        );
        let mut outer = dx_archive::TarWriter::new();
        outer.add_file("metadata.gz", b"x\n", 0o644).expect("outer");
        std::fs::write(&gem, outer.finish_tar().expect("tar")).expect("gem");
        let error = gem_file_list(&gem).expect_err("no data archive");
        assert!(
            matches!(error, RubygemsError::NoDataArchive { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("has no data.tar.gz"), "{error}");
    }

    #[test]
    fn gem_file_list_refuses_a_dot_dot_member() {
        let scratch = scratch_dir();
        let gem = scratch.path().join("demo-1.2.3.gem");
        let data = raw_tar_gz("../lib/demo.rb", b"x\n");
        let mut outer = dx_archive::TarWriter::new();
        outer.add_file("data.tar.gz", &data, 0o644).expect("outer");
        std::fs::write(&gem, outer.finish_tar().expect("tar")).expect("gem");
        let error = gem_file_list(&gem).expect_err("dot dot");
        assert!(error.to_string().contains("'..'"), "{error}");
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
        write_script(&fake, &format!("#!/bin/sh\n{body}\n"));
        fake
    }

    #[cfg(unix)]
    fn write_script(path: &Path, text: &str) {
        use std::os::unix::fs::PermissionsExt;
        let _spawn = dx_testing::spawning();
        std::fs::write(path, text).expect("write script");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    #[cfg(unix)]
    fn emit_file(path: &Path) -> String {
        use std::fmt::Write as _;
        let mut escaped = String::new();
        for byte in std::fs::read(path).expect("read gem") {
            write!(escaped, "\\{byte:03o}").expect("format");
        }
        format!("printf '%b' '{escaped}' > \"$4\"")
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
            &format!("[ \"$1\" = build ] && {}", emit_file(&prebuilt)),
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
            &format!("[ \"$1\" = build ] && {}", emit_file(&wrong)),
        );
        let error =
            rubygems_build_checked(&bad, &mapped, &scratch.path().join("s2"), "demo", "1.2.3")
                .expect_err("file list refused");
        assert!(
            matches!(error, RubygemsError::FileListMismatch { .. }),
            "{error:?}"
        );
        assert!(
            error.to_string().contains("unexpected [\"secret.rb\"]"),
            "{error}"
        );

        let push = fake_gem(
            &bin,
            "echo \"Repushing of gem versions is not allowed $GEM_HOST_API_KEY\" >&2\nexit 1",
        );
        let error = gem_push(&push, &gem, "s3cret").expect_err("push fails");
        assert!(
            matches!(error, RubygemsError::GemFailed { .. }),
            "{error:?}"
        );
        let message = error.to_string();
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
