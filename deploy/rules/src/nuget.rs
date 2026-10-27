//! NuGet publisher: feed staging, nuspec identity checks and the dotnet push.

use crate::archive::*;
use crate::run::{
    deploy_profile, env_or_empty, scrubbed_command, DeployRun, OwnerGate, DOTNET_CHILD_ENV_EXTRA,
    PUBLISH_CHILD_ENV_KEEP,
};
use crate::stage::*;
use std::io;
use std::path::Path;

/// Returns one NuGet version with the NuGet normalization applied.
pub fn nuget_normalize_version(raw: &str) -> Result<String, String> {
    let text = raw.trim();
    let without_meta = text.split('+').next().unwrap_or("");
    let (core, release) = match without_meta.split_once('-') {
        Some((core, release)) => (core, Some(release)),
        None => (without_meta, None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() > 4 {
        return Err(format!(
            "invalid NuGet version {text:?}: want at most four numeric parts"
        ));
    }
    let mut numbers = [0u64; 4];
    for (slot, part) in numbers.iter_mut().zip(&parts) {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!(
                "invalid NuGet version {text:?}: want numeric version parts"
            ));
        }
        *slot = part
            .parse()
            .map_err(|_| format!("invalid NuGet version {text:?}: version part too large"))?;
    }
    let mut out = format!("{}.{}.{}", numbers[0], numbers[1], numbers[2]);
    if numbers[3] != 0 {
        out.push_str(&format!(".{}", numbers[3]));
    }
    if let Some(release) = release {
        let valid = !release.is_empty()
            && release.split('.').all(|label| {
                !label.is_empty()
                    && label
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            });
        if !valid {
            return Err(format!(
                "invalid NuGet version {text:?}: bad prerelease label"
            ));
        }
        out.push('-');
        out.push_str(&release.to_ascii_lowercase());
    }
    Ok(out)
}

/// Reads the id and version declared in one nuspec document.
fn nuget_parse_nuspec(xml: &str) -> Result<(String, String), String> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut id: Vec<String> = Vec::new();
    let mut version: Vec<String> = Vec::new();
    loop {
        let event = reader
            .read_event()
            .map_err(|error| format!("nuspec is not valid XML: {error}"))?;
        match event {
            Event::Start(start) => {
                stack.push(String::from_utf8_lossy(start.local_name().as_ref()).into_owned());
                if let Some(list) = nuget_identity_slot(&stack, &mut id, &mut version) {
                    list.push(String::new());
                }
            }
            Event::Empty(start) => {
                stack.push(String::from_utf8_lossy(start.local_name().as_ref()).into_owned());
                if let Some(list) = nuget_identity_slot(&stack, &mut id, &mut version) {
                    list.push(String::new());
                }
                stack.pop();
            }
            Event::End(_) => {
                stack.pop();
            }
            Event::Text(text) => {
                if let Some(list) = nuget_identity_slot(&stack, &mut id, &mut version) {
                    let text = text
                        .xml10_content()
                        .map_err(|error| format!("nuspec text is not valid: {error}"))?;
                    if let Some(last) = list.last_mut() {
                        last.push_str(&text);
                    }
                }
            }
            Event::CData(text) => {
                if let Some(list) = nuget_identity_slot(&stack, &mut id, &mut version) {
                    let text = text
                        .decode()
                        .map_err(|error| format!("nuspec text is not valid: {error}"))?;
                    if let Some(last) = list.last_mut() {
                        last.push_str(&text);
                    }
                }
            }
            Event::GeneralRef(_) => {
                if nuget_identity_slot(&stack, &mut id, &mut version).is_some() {
                    return Err("nuspec id or version contains an entity reference".to_owned());
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let one = |name: &str, values: Vec<String>| -> Result<String, String> {
        match values.len() {
            0 => Err(format!("nuspec has no <{name}> in <metadata>")),
            1 => {
                let value = values[0].trim().to_owned();
                if value.is_empty() {
                    Err(format!("nuspec <{name}> is empty"))
                } else {
                    Ok(value)
                }
            }
            _ => Err(format!("nuspec has more than one <{name}> in <metadata>")),
        }
    };
    Ok((one("id", id)?, one("version", version)?))
}

fn nuget_identity_slot<'a>(
    stack: &[String],
    id: &'a mut Vec<String>,
    version: &'a mut Vec<String>,
) -> Option<&'a mut Vec<String>> {
    match stack {
        [a, b, c] if a == "package" && b == "metadata" && c == "id" => Some(id),
        [a, b, c] if a == "package" && b == "metadata" && c == "version" => Some(version),
        _ => None,
    }
}

/// Reads the package id and normalized version embedded in one nupkg.
pub fn nuget_read_identity(nupkg: &Path) -> Result<(String, String), String> {
    use std::io::Read;
    let file = std::fs::File::open(nupkg)
        .map_err(|error| format!("cannot open {}: {error}", nupkg.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| format!("{} is not a valid nupkg archive: {error}", nupkg.display()))?;
    let mut names = Vec::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index_raw(index)
            .map_err(|error| format!("{} has an unreadable entry: {error}", nupkg.display()))?;
        let name = entry
            .name()
            .map_err(|error| format!("{} has an unreadable entry name: {error}", nupkg.display()))?
            .into_owned();
        if !name.contains('/') && name.to_ascii_lowercase().ends_with(".nuspec") {
            names.push(name);
        }
    }
    let name = match names.as_slice() {
        [] => return Err(format!("{} has no root .nuspec", nupkg.display())),
        [one] => one.clone(),
        _ => {
            return Err(format!(
                "{} has more than one root .nuspec: {}",
                nupkg.display(),
                names.join(", ")
            ))
        }
    };
    let mut xml = String::new();
    archive
        .by_name(&name)
        .map_err(|error| format!("cannot read {name}: {error}"))?
        .take(1 << 20)
        .read_to_string(&mut xml)
        .map_err(|error| format!("cannot read {name}: {error}"))?;
    let (id, version) = nuget_parse_nuspec(&xml).map_err(|error| format!("{name}: {error}"))?;
    if id.len() > 100
        || id.starts_with(['.', '-', '_'])
        || id.ends_with(['.', '-', '_'])
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        return Err(format!("{name}: invalid NuGet package id {id:?}"));
    }
    let version = nuget_normalize_version(&version).map_err(|error| format!("{name}: {error}"))?;
    Ok((id, version))
}

/// Checks one packaged identity against the declared package id and version.
pub fn nuget_check_identity(
    actual: &(String, String),
    expected_id: &str,
    expected_version: &str,
) -> Result<(), String> {
    if !expected_id.is_empty() && !actual.0.eq_ignore_ascii_case(expected_id) {
        return Err(format!(
            "package id mismatch: nupkg declares {}, BUILD declares {expected_id}",
            actual.0
        ));
    }
    if !expected_version.is_empty() {
        let want = nuget_normalize_version(expected_version)?;
        if want != actual.1 {
            return Err(format!(
                "package version mismatch: nupkg declares {}, BUILD declares {expected_version}",
                actual.1
            ));
        }
    }
    Ok(())
}

/// Builds one NuGet folder feed.
pub fn nuget_build_feed(
    nupkg_src: &Path,
    outdir: &Path,
    feed_name: &str,
) -> io::Result<std::path::PathBuf> {
    let feed = outdir.join(feed_name.to_owned() + "-feed");
    std::fs::create_dir_all(&feed)?;
    let base = basename(nupkg_src)?;
    let dest = feed.join(&base);
    copy_verified(nupkg_src, &dest)?;
    Ok(feed)
}

/// Owner approval gate for nuget_deploy.
pub(crate) const NUGET_GATE: OwnerGate = OwnerGate {
    tool: "nuget_deploy",
    action: "push",
    credentials: &["NUGET_API_KEY"],
    approved: "NUGET_PUBLISH_APPROVED",
    live: "NUGET_PUBLISH_LIVE",
};

/// Lowest NuGet version that reads the push key from `NUGET_API_KEY`.
const NUGET_ENV_KEY_MIN: (u32, u32) = (7, 6);

/// Reads `MAJOR.MINOR` from the last non-empty line of `dotnet nuget --version` output.
fn nuget_parse_version(output: &str) -> Option<(u32, u32)> {
    let last = output.lines().rev().find(|line| !line.trim().is_empty())?;
    last.split_whitespace().find_map(|token| {
        let mut parts = token.split('.');
        let major = parts.next()?.parse::<u32>().ok()?;
        let minor_raw = parts.next()?;
        let digits: String = minor_raw.chars().take_while(char::is_ascii_digit).collect();
        Some((major, digits.parse::<u32>().ok()?))
    })
}

/// True when the reported NuGet version reads `NUGET_API_KEY` for push.
pub fn nuget_reads_api_key_env(version_output: &str) -> bool {
    nuget_parse_version(version_output).is_some_and(|version| version >= NUGET_ENV_KEY_MIN)
}

/// Pushes a nupkg with the key in the child env only, never in argv.
pub fn nuget_live_push(
    dotnet: &Path,
    nupkg: &Path,
    source: &str,
    token: &str,
    id: &str,
    version: &str,
) -> i32 {
    let mut probe = scrubbed_command(
        dotnet,
        &[PUBLISH_CHILD_ENV_KEEP, DOTNET_CHILD_ENV_EXTRA],
        &[
            ("DOTNET_NOLOGO", "1"),
            ("DOTNET_SKIP_FIRST_TIME_EXPERIENCE", "1"),
        ],
    );
    probe.arg("nuget").arg("--version");
    let reported = match probe.output() {
        Ok(out) => String::from_utf8_lossy(&out.stdout).into_owned(),
        Err(error) => {
            eprintln!("nuget_deploy: {error}");
            return 1;
        }
    };
    if !nuget_reads_api_key_env(&reported) {
        eprintln!(
            "nuget_deploy: NuGet {} does not read NUGET_API_KEY; need NuGet {}.{}+ (.NET SDK 10.0.300+); refusing to put the key on the command line",
            reported.lines().last().unwrap_or("").trim(),
            NUGET_ENV_KEY_MIN.0,
            NUGET_ENV_KEY_MIN.1
        );
        return 1;
    }
    let mut cmd = scrubbed_command(
        dotnet,
        &[PUBLISH_CHILD_ENV_KEEP, DOTNET_CHILD_ENV_EXTRA],
        &[("NUGET_API_KEY", token)],
    );
    cmd.arg("nuget")
        .arg("push")
        .arg(nupkg)
        .arg("--source")
        .arg(source)
        .arg("--skip-duplicate");
    match cmd.status() {
        Ok(status) if status.success() => {
            println!("nuget_deploy: pushed {id} {version}");
            0
        }
        Ok(status) => {
            eprintln!("nuget_deploy: dotnet failed with {status}");
            1
        }
        Err(error) => {
            eprintln!("nuget_deploy: {error}");
            1
        }
    }
}

/// Runs the NuGet deploy launcher.
pub fn nuget_main(
    nupkg_rloc: &str,
    target_name: &str,
    expected_id: &str,
    expected_version: &str,
    package_source: &str,
    argv: &[String],
) -> i32 {
    let Some(run) = DeployRun::start(
        "nuget_deploy",
        "the feed is exactly the file pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let outdir = run.outdir();
    let Some(nupkg) = run.resolve(nupkg_rloc) else {
        return 1;
    };
    let identity = match nuget_read_identity(&nupkg).and_then(|identity| {
        nuget_check_identity(&identity, expected_id, expected_version).map(|()| identity)
    }) {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!("nuget_deploy: {target_name}: {error}");
            return 1;
        }
    };
    let (package_id, package_version) = (identity.0.as_str(), identity.1.as_str());
    if NUGET_GATE.requested(env_or_empty) {
        if package_version == "0.0.0" {
            eprintln!(
                "nuget_deploy: live push refuses {package_id} version {package_version}; set a real version in the nuspec"
            );
            return 1;
        }
        let secrets = match NUGET_GATE.authorize(env_or_empty) {
            Ok(secrets) => secrets,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        let Some(dotnet) = which_on_path("dotnet") else {
            eprintln!("nuget_deploy: 'dotnet' CLI not found on PATH");
            return 1;
        };
        return nuget_live_push(
            &dotnet,
            &nupkg,
            package_source,
            secrets.get("NUGET_API_KEY"),
            package_id,
            package_version,
        );
    }
    match nuget_build_feed(&nupkg, outdir, target_name) {
        Ok(feed) => {
            let profile = deploy_profile();
            println!("nuget_deploy: staged {package_id} {package_version} (profile: {profile})");
            println!("  feed: {}", feed.display());
            if let Ok(base) = basename(&nupkg) {
                println!("  nupkg: {}", feed.join(base).display());
            }
            0
        }
        Err(error) => {
            eprintln!("nuget_deploy: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    fn deploy_env<R>(live: bool, f: impl FnOnce() -> R) -> R {
        let mut vars: Vec<(&str, Option<&str>)> = vec![
            ("RUNFILES_DIR", None),
            ("RUNFILES_MANIFEST_FILE", None),
            ("NUGET_API_KEY", None),
            ("NUGET_PUBLISH_APPROVED", None),
            ("NUGET_PUBLISH_LIVE", None),
        ];
        if live {
            vars[2] = ("NUGET_API_KEY", Some("secret-token"));
            vars[3] = ("NUGET_PUBLISH_APPROVED", Some("1"));
            vars[4] = ("NUGET_PUBLISH_LIVE", Some("1"));
        }
        with_env(&vars, f)
    }

    #[test]
    fn nuget_feed_copies_nupkg() {
        let scratch = scratch_dir();
        let nupkg = scratch.path().join("nuget_demo.0.0.0.nupkg");
        std::fs::write(&nupkg, b"nuget-demo-nupkg-bytes-v1").expect("write nupkg");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let feed = nuget_build_feed(&nupkg, &outdir, "nuget_demo").expect("feed");
        assert!(feed.join("nuget_demo.0.0.0.nupkg").is_file());
    }

    #[test]
    fn nuget_version_normalization_follows_nuget() {
        let norm = |v: &str| nuget_normalize_version(v).expect("version");
        assert_eq!(norm("1.0"), "1.0.0");
        assert_eq!(norm("01.2.3.0"), "1.2.3");
        assert_eq!(norm("1.2.3.4"), "1.2.3.4");
        assert_eq!(norm("1.0.0-BETA.1+build5"), "1.0.0-beta.1");
        assert_eq!(norm("0.0.0.0+meta"), "0.0.0");
        assert!(nuget_normalize_version("1.x.0").is_err());
        assert!(nuget_normalize_version("1.2.3.4.5").is_err());
        assert!(nuget_normalize_version("1.0.0-").is_err());
        assert!(nuget_normalize_version("").is_err());
    }

    #[test]
    fn nuget_identity_reads_the_packaged_nuspec() {
        let scratch = scratch_dir();
        let nupkg = scratch.path().join("anything.nupkg");
        write_nupkg(
            &nupkg,
            &[
                ("Dx.Plugin.nuspec", &nuspec("Dx.Plugin", "1.2.0.0")),
                ("lib/a.dll", "x"),
            ],
        );
        let identity = nuget_read_identity(&nupkg).expect("identity");
        assert_eq!(identity, ("Dx.Plugin".to_owned(), "1.2.0".to_owned()));
        assert!(nuget_check_identity(&identity, "dx.plugin", "1.2").is_ok());
        assert!(nuget_check_identity(&identity, "", "").is_ok());
        let id_error = nuget_check_identity(&identity, "Other", "").expect_err("id");
        assert!(id_error.contains("package id mismatch"), "{id_error}");
        let version_error = nuget_check_identity(&identity, "", "2.0.0").expect_err("version");
        assert!(
            version_error.contains("package version mismatch"),
            "{version_error}"
        );
    }

    #[test]
    fn nuget_identity_rejects_bad_packages() {
        let scratch = scratch_dir();
        let path = scratch.path().join("bad.nupkg");
        std::fs::write(&path, b"not a zip").expect("write");
        assert!(nuget_read_identity(&path)
            .expect_err("zip")
            .contains("not a valid nupkg"));
        write_nupkg(&path, &[("lib/a.dll", "x")]);
        assert!(nuget_read_identity(&path)
            .expect_err("none")
            .contains("no root .nuspec"));
        write_nupkg(
            &path,
            &[
                ("a.nuspec", &nuspec("A", "1.0.0")),
                ("b.nuspec", &nuspec("B", "1.0.0")),
            ],
        );
        assert!(nuget_read_identity(&path)
            .expect_err("two")
            .contains("more than one root .nuspec"));
        write_nupkg(
            &path,
            &[(
                "a.nuspec",
                "<package><metadata><id>A</id></metadata></package>",
            )],
        );
        assert!(nuget_read_identity(&path)
            .expect_err("no version")
            .contains("no <version>"));
        write_nupkg(
            &path,
            &[("a.nuspec", "<package><metadata><id>A</id><id>B</id><version>1.0.0</version></metadata></package>")],
        );
        assert!(nuget_read_identity(&path)
            .expect_err("dup")
            .contains("more than one <id>"));
        write_nupkg(&path, &[("a.nuspec", "<package><metadata><id>A</id>")]);
        assert!(nuget_read_identity(&path).is_err());
        write_nupkg(&path, &[("a.nuspec", &nuspec("A/../B", "1.0.0"))]);
        assert!(nuget_read_identity(&path)
            .expect_err("id chars")
            .contains("invalid NuGet package id"));
        write_nupkg(&path, &[("a.nuspec", &nuspec("A", "one"))]);
        assert!(nuget_read_identity(&path)
            .expect_err("version")
            .contains("invalid NuGet version"));
    }

    #[test]
    fn nuget_main_stages_and_reports_the_packaged_identity() {
        let scratch = scratch_dir();
        let nupkg = scratch.path().join("pkg.nupkg");
        write_nupkg(&nupkg, &[("Real.Pkg.nuspec", &nuspec("Real.Pkg", "2.0.0"))]);
        let outdir = scratch.path().join("out");
        let argv = vec!["launcher".to_owned(), outdir.display().to_string()];
        let rloc = nupkg.display().to_string();
        deploy_env(false, || {
            assert_eq!(
                nuget_main(
                    &rloc,
                    "publish",
                    "Real.Pkg",
                    "2.0",
                    "https://nuget.invalid",
                    &argv
                ),
                0
            );
            assert_eq!(
                nuget_main(&rloc, "publish", "", "", "https://nuget.invalid", &argv),
                0
            );
            assert_eq!(
                nuget_main(
                    &rloc,
                    "publish",
                    "publish",
                    "",
                    "https://nuget.invalid",
                    &argv
                ),
                1
            );
            assert_eq!(
                nuget_main(
                    &rloc,
                    "publish",
                    "",
                    "9.9.9",
                    "https://nuget.invalid",
                    &argv
                ),
                1
            );
        });
        let staged = outdir.join("publish-feed/pkg.nupkg");
        assert_eq!(
            std::fs::read(staged).expect("staged"),
            std::fs::read(&nupkg).expect("src")
        );
    }

    #[test]
    fn nuget_live_push_checks_the_actual_package_version() {
        let scratch = scratch_dir();
        let nupkg = scratch.path().join("placeholder.nupkg");
        write_nupkg(&nupkg, &[("P.nuspec", &nuspec("P", "0.0.0"))]);
        let argv = vec![
            "launcher".to_owned(),
            scratch.path().join("out").display().to_string(),
        ];
        let rloc = nupkg.display().to_string();
        deploy_env(true, || {
            assert_eq!(
                nuget_main(&rloc, "p", "", "1.0.0", "https://nuget.invalid", &argv),
                1,
                "a configured version cannot hide the packaged version"
            );
            assert_eq!(
                nuget_main(&rloc, "p", "", "", "https://nuget.invalid", &argv),
                1
            );
        });
    }

    #[cfg(unix)]
    fn fake_dotnet(dir: &Path, nuget_version: &str, exit: i32) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join("dotnet");
        let d = dir.display();
        let script = format!(
            "#!/bin/sh\nif [ \"$2\" = \"--version\" ]; then printf 'NuGet Command Line\\n{nuget_version}\\n'; exit 0; fi\nprintf '%s\\n' \"$@\" > '{d}/args.txt'\nenv > '{d}/env.txt'\nexit {exit}\n"
        );
        std::fs::write(&path, script).expect("write dotnet");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    }

    #[cfg(unix)]
    #[test]
    fn nuget_live_push_keeps_the_key_out_of_argv() {
        let scratch = scratch_dir();
        let dotnet = fake_dotnet(scratch.path(), "7.6.0", 0);
        with_env(&[("DX_LEAK_PROBE", Some("1"))], || {
            assert_eq!(
                nuget_live_push(
                    &dotnet,
                    Path::new("a.nupkg"),
                    "https://n.invalid",
                    "sekret",
                    "a",
                    "1.0.0"
                ),
                0
            );
        });
        let args = std::fs::read_to_string(scratch.path().join("args.txt")).expect("args");
        let env = std::fs::read_to_string(scratch.path().join("env.txt")).expect("env");
        assert!(args.contains("push") && args.contains("--skip-duplicate"));
        assert!(!args.contains("sekret") && !args.contains("--api-key"));
        assert!(env.contains("NUGET_API_KEY=sekret"));
        assert!(!env.contains("DX_LEAK_PROBE"));
    }

    #[cfg(unix)]
    #[test]
    fn nuget_live_push_refuses_nuget_older_than_7_6() {
        let scratch = scratch_dir();
        let dotnet = fake_dotnet(scratch.path(), "6.14.0", 0);
        assert_eq!(
            nuget_live_push(
                &dotnet,
                Path::new("a.nupkg"),
                "https://n.invalid",
                "sekret",
                "a",
                "1.0.0"
            ),
            1
        );
        assert!(!scratch.path().join("args.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn nuget_live_push_surfaces_tool_failure() {
        let scratch = scratch_dir();
        let dotnet = fake_dotnet(scratch.path(), "7.6.0", 1);
        assert_eq!(
            nuget_live_push(
                &dotnet,
                Path::new("a.nupkg"),
                "https://n.invalid",
                "sekret",
                "a",
                "1.0.0"
            ),
            1
        );
    }

    #[test]
    fn nuget_api_key_env_needs_nuget_7_6() {
        assert!(!nuget_reads_api_key_env("6.12.1"));
        assert!(!nuget_reads_api_key_env("7.5.9"));
        assert!(nuget_reads_api_key_env("7.6.0"));
        assert!(nuget_reads_api_key_env("7.6.0-preview.1"));
        assert!(nuget_reads_api_key_env("NuGet Command Line\n7.6.0\n"));
        assert!(nuget_reads_api_key_env("8.0"));
        assert!(!nuget_reads_api_key_env(
            "Welcome to .NET 9.0!\nSDK Version: 9.0.100\n\nNuGet Command Line\n6.12.1\n"
        ));
        assert!(!nuget_reads_api_key_env(""));
        assert!(!nuget_reads_api_key_env("garbage"));
    }
}
