//! PyPI publisher: wheelhouse staging and the twine upload.

use crate::archive::*;
use crate::run::{
    deploy_profile, env_or_empty, scrubbed_command, DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP,
};
use crate::stage::*;
use std::io;
use std::path::Path;

/// Returns one distribution name with PEP 503 normalization applied.
pub fn pypi_normalize_name(raw: &str) -> String {
    let mut out = String::new();
    let mut pending = false;
    for c in raw.chars() {
        if matches!(c, '.' | '_' | '-') {
            pending = true;
            continue;
        }
        if pending && !out.is_empty() {
            out.push('-');
        }
        pending = false;
        out.push(c.to_ascii_lowercase());
    }
    out
}

fn pypi_valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && bytes[0].is_ascii_alphanumeric()
        && bytes[bytes.len() - 1].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// Reads the distribution name and version from one wheel or sdist filename.
pub fn pypi_artifact_identity(base: &str) -> Result<(String, String), String> {
    let (name, version) = if let Some(stem) = base.strip_suffix(".whl") {
        let parts: Vec<&str> = stem.split('-').collect();
        if parts.len() < 5 || parts.len() > 6 {
            return Err(format!(
                "pypi_deploy: invalid wheel filename {base:?}: want name-version-python-abi-platform.whl"
            ));
        }
        (parts[0], parts[1])
    } else if let Some(stem) = base.strip_suffix(".tar.gz") {
        stem.rsplit_once('-').ok_or_else(|| {
            format!("pypi_deploy: invalid sdist filename {base:?}: want name-version.tar.gz")
        })?
    } else {
        return Err(format!(
            "pypi_deploy: unsupported artifact {base:?}: want a .whl or .tar.gz"
        ));
    };
    if !pypi_valid_name(name) || version.is_empty() {
        return Err(format!(
            "pypi_deploy: invalid distribution name or version in {base:?}"
        ));
    }
    Ok((pypi_normalize_name(name), version.to_owned()))
}

const NATIVE_SUFFIXES: &[&str] = &[".so", ".pyd", ".dylib"];
const BUILD_ONLY_SUFFIXES: &[&str] = &[".h", ".a", ".lib", ".obj", ".o", ".pdb"];

/// Reads the RECORD of one wheel and rejects native code in a none-any wheel and build-only files.
fn pypi_check_wheel_contents(wheel: &Path) -> Result<(), String> {
    use std::io::Read;
    let name = basename(wheel).map_err(|error| error.to_string())?;
    let file = std::fs::File::open(wheel)
        .map_err(|error| format!("pypi_deploy: cannot open wheel {name}: {error}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| format!("pypi_deploy: wheel {name} is not a zip file: {error}"))?;
    let record_name = (0..archive.len())
        .filter_map(|index| {
            let name = archive.name_for_index(index)?.ok()?;
            Some(name.into_owned())
        })
        .find(|entry| entry.ends_with(".dist-info/RECORD") && entry.matches('/').count() == 1)
        .ok_or_else(|| format!("pypi_deploy: wheel {name} has no .dist-info/RECORD"))?;
    let mut record = String::new();
    archive
        .by_name(&record_name)
        .and_then(|mut entry| entry.read_to_string(&mut record).map_err(Into::into))
        .map_err(|error| {
            format!("pypi_deploy: cannot read {record_name} in wheel {name}: {error}")
        })?;
    let none_any = name.ends_with("-none-any.whl");
    let tags = FilenameTags::parse(&name);
    let mut has_native = false;
    for line in record.lines() {
        let path = line.split(',').next().unwrap_or("");
        let lower = path.to_ascii_lowercase();
        let native = NATIVE_SUFFIXES.iter().any(|suffix| lower.ends_with(suffix));
        has_native |= native;
        if native {
            if let Some(tags) = &tags {
                tags.check_native(&name, path, &lower)?;
            }
        }
        if none_any && NATIVE_SUFFIXES.iter().any(|suffix| lower.ends_with(suffix)) {
            return Err(format!(
                "pypi_deploy: wheel {name} is tagged none-any but contains {path}"
            ));
        }
        if BUILD_ONLY_SUFFIXES
            .iter()
            .any(|suffix| lower.ends_with(suffix))
        {
            return Err(format!(
                "pypi_deploy: wheel {name} ships build-only file {path}"
            ));
        }
    }
    if let Some(tags) = tags {
        if has_native || !tags.platforms.iter().all(|platform| platform == "any") {
            let wheel_name = record_name.replace("RECORD", "WHEEL");
            let mut metadata = String::new();
            archive
                .by_name(&wheel_name)
                .and_then(|mut entry| entry.read_to_string(&mut metadata).map_err(Into::into))
                .map_err(|error| {
                    format!("pypi_deploy: cannot read {wheel_name} in wheel {name}: {error}")
                })?;
            tags.check_metadata(&name, &metadata)?;
        }
    }
    Ok(())
}

/// The python, abi and platform tags from a wheel filename, with dotted forms expanded.
struct FilenameTags {
    pythons: Vec<String>,
    abis: Vec<String>,
    platforms: Vec<String>,
}

impl FilenameTags {
    fn parse(name: &str) -> Option<Self> {
        let stem = name.strip_suffix(".whl")?;
        let mut fields = stem.rsplit('-');
        let platform = fields.next()?;
        let abi = fields.next()?;
        let python = fields.next()?;
        let split = |field: &str| field.split('.').map(str::to_owned).collect();
        Some(Self {
            pythons: split(python),
            abis: split(abi),
            platforms: split(platform),
        })
    }

    fn abi_label(&self) -> String {
        self.abis.join(".")
    }

    fn check_metadata(&self, name: &str, metadata: &str) -> Result<(), String> {
        let declared: Vec<&str> = metadata
            .lines()
            .filter_map(|line| line.strip_prefix("Tag:"))
            .map(str::trim)
            .collect();
        for python in &self.pythons {
            for abi in &self.abis {
                for platform in &self.platforms {
                    let tag = format!("{python}-{abi}-{platform}");
                    if !declared.contains(&tag.as_str()) {
                        return Err(format!(
                            "pypi_deploy: wheel {name} is named {tag} but its WHEEL file does not list that tag"
                        ));
                    }
                }
            }
        }
        let platform_any = self.platforms.iter().all(|platform| platform == "any");
        let purelib = metadata
            .lines()
            .filter_map(|line| line.strip_prefix("Root-Is-Purelib:"))
            .any(|value| value.trim().eq_ignore_ascii_case("true"));
        if !platform_any && purelib {
            return Err(format!(
                "pypi_deploy: wheel {name} has a platform tag but its WHEEL file says Root-Is-Purelib: true"
            ));
        }
        Ok(())
    }

    fn check_native(&self, name: &str, path: &str, lower: &str) -> Result<(), String> {
        let abi = self.abi_label();
        let on = |needle: &str| {
            self.platforms
                .iter()
                .any(|platform| platform.contains(needle))
        };
        if lower.ends_with(".pyd") && on("linux") {
            return Err(format!(
                "pypi_deploy: wheel {name} is tagged {} but contains {path}",
                self.platforms.join(".")
            ));
        }
        if lower.ends_with(".so") && self.platforms.iter().any(|p| p.starts_with("win")) {
            return Err(format!(
                "pypi_deploy: wheel {name} is tagged {} but contains {path}",
                self.platforms.join(".")
            ));
        }
        if let Some(built) = path_python_tag(lower) {
            if !self.abis.contains(&built) {
                return Err(format!(
                    "pypi_deploy: wheel {name} is tagged {abi} but contains {path} built for {built}"
                ));
            }
        } else if lower.contains(".abi3.") && !self.abis.iter().any(|tag| tag == "abi3") {
            return Err(format!(
                "pypi_deploy: wheel {name} is tagged {abi} but contains {path} built for abi3"
            ));
        }
        Ok(())
    }
}

/// The `cp3NN` tag in an extension module name like `_core.cpython-312-x86_64-linux-gnu.so`.
fn path_python_tag(lower: &str) -> Option<String> {
    let file = lower.rsplit('/').next()?;
    for marker in [".cpython-3", ".cp3"] {
        if let Some(index) = file.find(marker) {
            let rest = &file[index + marker.len()..];
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if !digits.is_empty() && rest[digits.len()..].starts_with('-') {
                return Some(format!("cp3{digits}"));
            }
        }
    }
    None
}

/// Checks the wheel and sdist name the same project and version.
fn pypi_check_pair(wheel_src: &Path, sdist_src: Option<&Path>) -> io::Result<(String, String)> {
    let invalid = |message: String| io::Error::new(io::ErrorKind::InvalidData, message);
    let (project, version) = pypi_artifact_identity(&basename(wheel_src)?).map_err(invalid)?;
    pypi_check_wheel_contents(wheel_src).map_err(invalid)?;
    if let Some(sdist) = sdist_src {
        let (sdist_project, sdist_version) =
            pypi_artifact_identity(&basename(sdist)?).map_err(invalid)?;
        if sdist_project != project || sdist_version != version {
            return Err(invalid(format!(
                "pypi_deploy: wheel {} is {project} {version} but sdist {} is {sdist_project} {sdist_version}",
                basename(wheel_src)?,
                basename(sdist)?,
            )));
        }
    }
    Ok((project, version))
}

/// Builds one PyPI wheelhouse.
pub fn pypi_build_wheelhouse(
    wheel_src: &Path,
    sdist_src: Option<&Path>,
    outdir: &Path,
    dist_name: &str,
) -> io::Result<std::path::PathBuf> {
    let (project, _) = pypi_check_pair(wheel_src, sdist_src)?;
    let house = outdir.join(dist_name.to_owned() + "-wheelhouse");
    std::fs::create_dir_all(&house)?;
    let mut entries: Vec<(String, String)> = Vec::new();
    let mut sources: Vec<&Path> = vec![wheel_src];
    if let Some(sdist) = sdist_src {
        sources.push(sdist);
    }
    for src in sources {
        let base = basename(src)?;
        let dest = house.join(&base);
        let digest = copy_verified(src, &dest)?;
        entries.push((base, digest));
    }
    entries.sort();
    let index_dir = house.join("simple-index").join(&project);
    std::fs::create_dir_all(&index_dir)?;
    let mut body = String::from("<!DOCTYPE html>\n<html><body>\n");
    for (base, digest) in &entries {
        body.push_str(&format!(
            "<a href=\"../../{base}#sha256={digest}\">{base}</a><br/>\n"
        ));
    }
    body.push_str("</body></html>\n");
    std::fs::write(index_dir.join("index.html"), body.as_bytes())?;
    Ok(house)
}

/// Owner approval gate for pypi_deploy.
pub(crate) const PYPI_GATE: OwnerGate = OwnerGate {
    tool: "pypi_deploy",
    action: "upload",
    credentials: &["PYPI_API_TOKEN"],
    approved: "PYPI_PUBLISH_APPROVED",
    live: "PYPI_PUBLISH_LIVE",
};

/// Extra env names twine keeps beyond the publish base list.
const TWINE_CHILD_ENV_EXTRA: &[&str] = &["REQUESTS_CA_BUNDLE"];

fn pypi_scrub(text: &[u8], token: &str) -> String {
    crate::run::pages_redact(&String::from_utf8_lossy(text), token)
}

fn pypi_duplicate_note(output: &str, base: &str, version: &str, url: &str) -> Option<String> {
    output
        .to_ascii_lowercase()
        .contains("already exists")
        .then(|| {
            format!(
                "pypi_deploy: {base} version {version} is already on {url}; upload a new version"
            )
        })
}

/// Checks and uploads verified copies of the wheel and sdist with twine.
fn pypi_upload(
    twine: &Path,
    wheel: &Path,
    sdist: Option<&Path>,
    stage: &Path,
    repository_url: &str,
    token: &str,
) -> i32 {
    let (_, version) = match pypi_check_pair(wheel, sdist) {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!("pypi_deploy: {error}");
            return 1;
        }
    };
    let code = pypi_upload_staged(twine, wheel, sdist, stage, repository_url, token, &version);
    let _ = std::fs::remove_dir_all(stage);
    code
}

fn pypi_upload_staged(
    twine: &Path,
    wheel: &Path,
    sdist: Option<&Path>,
    stage: &Path,
    repository_url: &str,
    token: &str,
    version: &str,
) -> i32 {
    let mut files = Vec::new();
    for src in std::iter::once(wheel).chain(sdist) {
        let copied = basename(src).and_then(|base| {
            std::fs::create_dir_all(stage)?;
            let dest = stage.join(base);
            copy_verified(src, &dest)?;
            Ok(dest)
        });
        match copied {
            Ok(dest) => files.push(dest),
            Err(error) => {
                eprintln!("pypi_deploy: {error}");
                return 1;
            }
        }
    }
    let command = |verb: &str| {
        let mut cmd = scrubbed_command(
            twine,
            &[PUBLISH_CHILD_ENV_KEEP, TWINE_CHILD_ENV_EXTRA],
            &[("TWINE_USERNAME", "__token__"), ("TWINE_PASSWORD", token)],
        );
        cmd.arg(verb);
        cmd
    };
    let mut check = command("check");
    check.arg("--strict").args(&files);
    match check.output() {
        Ok(out) if out.status.success() => {}
        Ok(out) => {
            eprint!("{}", pypi_scrub(&out.stdout, token));
            eprint!("{}", pypi_scrub(&out.stderr, token));
            eprintln!("pypi_deploy: twine check failed with {}", out.status);
            return 1;
        }
        Err(error) => {
            eprintln!("pypi_deploy: {error}");
            return 1;
        }
    }
    let mut upload = command("upload");
    upload
        .arg("--non-interactive")
        .arg("--repository-url")
        .arg(repository_url)
        .args(&files);
    let out = match upload.output() {
        Ok(out) => out,
        Err(error) => {
            eprintln!("pypi_deploy: {error}");
            return 1;
        }
    };
    let stdout = pypi_scrub(&out.stdout, token);
    let stderr = pypi_scrub(&out.stderr, token);
    print!("{stdout}");
    eprint!("{stderr}");
    let base = wheel.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if out.status.success() {
        println!("pypi_deploy: uploaded {base} to {repository_url}");
        return 0;
    }
    if let Some(note) =
        pypi_duplicate_note(&format!("{stdout}{stderr}"), base, version, repository_url)
    {
        eprintln!("{note}");
    }
    eprintln!("pypi_deploy: twine failed with {}", out.status);
    1
}

/// Runs the PyPI deploy tool.
pub fn pypi_main(
    wheel_rloc: &str,
    sdist_rloc: &str,
    dist_name: &str,
    repository_url: &str,
    argv: &[String],
) -> i32 {
    let Some(run) = DeployRun::start(
        "pypi_deploy",
        "the wheelhouse is exactly the files pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let outdir = run.outdir();
    let Some(wheel) = run.resolve(wheel_rloc) else {
        return 1;
    };
    let Some(sdist) = run.resolve_optional(sdist_rloc) else {
        return 1;
    };
    if PYPI_GATE.requested(env_or_empty) {
        let secrets = match PYPI_GATE.authorize(env_or_empty) {
            Ok(secrets) => secrets,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        return pypi_upload(
            Path::new("twine"),
            &wheel,
            sdist.as_deref(),
            &outdir.join(dist_name.to_owned() + "-upload"),
            repository_url,
            secrets.get("PYPI_API_TOKEN"),
        );
    }
    match pypi_build_wheelhouse(&wheel, sdist.as_deref(), outdir, dist_name) {
        Ok(house) => {
            let project = basename(&wheel)
                .and_then(|base| {
                    pypi_artifact_identity(&base)
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
                })
                .map(|(project, _)| project)
                .unwrap_or_default();
            let profile = deploy_profile();
            println!("pypi_deploy: staged {dist_name} (profile: {profile})");
            println!("  wheelhouse: {}", house.display());
            println!(
                "  index: {}",
                house
                    .join("simple-index")
                    .join(&project)
                    .join("index.html")
                    .display()
            );
            0
        }
        Err(error) => {
            eprintln!("pypi_deploy: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    fn wheel_bytes(files: &[&str]) -> Vec<u8> {
        wheel_bytes_with(files, None)
    }

    fn wheel_bytes_with(files: &[&str], wheel_file: Option<&str>) -> Vec<u8> {
        let mut record = String::from("pkg-1.0.dist-info/RECORD,,\n");
        for file in files {
            record.push_str(&format!("{file},sha256=x,1\n"));
        }
        let mut members = vec![("pkg-1.0.dist-info/RECORD", record.as_bytes(), 0o644)];
        if let Some(wheel_file) = wheel_file {
            members.push(("pkg-1.0.dist-info/WHEEL", wheel_file.as_bytes(), 0o644));
        }
        zip_archive(&members).expect("zip")
    }

    fn wheel_file(tags: &[&str], purelib: bool) -> String {
        let mut text = format!("Wheel-Version: 1.0\nRoot-Is-Purelib: {purelib}\n");
        for tag in tags {
            text.push_str(&format!("Tag: {tag}\n"));
        }
        text
    }

    fn wheel_error(name: &str, bytes: &[u8]) -> String {
        let scratch = scratch_dir();
        let wheel = scratch.path().join(name);
        std::fs::write(&wheel, bytes).expect("wheel");
        pypi_check_pair(&wheel, None)
            .expect_err("rejected")
            .to_string()
    }

    #[test]
    fn pypi_rejects_native_file_in_none_any_wheel() {
        let bytes = wheel_bytes(&["pkg/_core.so"]);
        assert_eq!(
            wheel_error("pkg-1.0-py3-none-any.whl", &bytes),
            "pypi_deploy: wheel pkg-1.0-py3-none-any.whl is tagged none-any but contains pkg/_core.so"
        );
        let scratch = scratch_dir();
        let wheel = scratch.path().join("pkg-1.0-cp312-cp312-linux_x86_64.whl");
        let tagged = wheel_file(&["cp312-cp312-linux_x86_64"], false);
        std::fs::write(&wheel, wheel_bytes_with(&["pkg/_core.so"], Some(&tagged))).expect("wheel");
        assert!(pypi_check_pair(&wheel, None).is_ok());
    }

    #[test]
    fn pypi_rejects_tag_missing_from_wheel_file() {
        let metadata = wheel_file(&["cp312-cp312-linux_x86_64"], false);
        let bytes = wheel_bytes_with(&["pkg/_core.so"], Some(&metadata));
        let error = wheel_error("pkg-1.0-cp311-cp311-linux_x86_64.whl", &bytes);
        assert!(error.contains("named cp311-cp311-linux_x86_64"), "{error}");
    }

    #[test]
    fn pypi_rejects_native_wheel_marked_purelib() {
        let metadata = wheel_file(&["cp312-cp312-linux_x86_64"], true);
        let bytes = wheel_bytes_with(&["pkg/_core.so"], Some(&metadata));
        let error = wheel_error("pkg-1.0-cp312-cp312-linux_x86_64.whl", &bytes);
        assert!(error.contains("Root-Is-Purelib: true"), "{error}");
    }

    #[test]
    fn pypi_rejects_extension_built_for_other_python() {
        let metadata = wheel_file(&["cp311-cp311-linux_x86_64"], false);
        let bytes = wheel_bytes_with(
            &["pkg/_core.cpython-312-x86_64-linux-gnu.so"],
            Some(&metadata),
        );
        assert_eq!(
            wheel_error("pkg-1.0-cp311-cp311-linux_x86_64.whl", &bytes),
            "pypi_deploy: wheel pkg-1.0-cp311-cp311-linux_x86_64.whl is tagged cp311 but contains pkg/_core.cpython-312-x86_64-linux-gnu.so built for cp312"
        );
    }

    #[test]
    fn pypi_accepts_windows_pyd_and_rejects_pyd_on_linux() {
        let scratch = scratch_dir();
        let windows = scratch.path().join("pkg-1.0-cp312-cp312-win_amd64.whl");
        let metadata = wheel_file(&["cp312-cp312-win_amd64"], false);
        let bytes = wheel_bytes_with(&["pkg/_core.cp312-win_amd64.pyd"], Some(&metadata));
        std::fs::write(&windows, &bytes).expect("wheel");
        assert!(pypi_check_pair(&windows, None).is_ok());
        let metadata = wheel_file(&["cp312-cp312-linux_x86_64"], false);
        let bytes = wheel_bytes_with(&["pkg/_core.pyd"], Some(&metadata));
        let error = wheel_error("pkg-1.0-cp312-cp312-linux_x86_64.whl", &bytes);
        assert!(error.contains("contains pkg/_core.pyd"), "{error}");
    }

    #[test]
    fn pypi_accepts_dotted_filename_tags_covered_by_wheel_file() {
        let scratch = scratch_dir();
        let wheel = scratch.path().join("pkg-1.0-py3-none-any.whl");
        let metadata = wheel_file(&["py2-none-any", "py3-none-any"], false);
        std::fs::write(&wheel, wheel_bytes_with(&[], Some(&metadata))).expect("wheel");
        assert!(pypi_check_pair(&wheel, None).is_ok());
    }

    #[test]
    fn pypi_rejects_build_only_file() {
        let bytes = wheel_bytes(&["pkg/include/pkg.h"]);
        assert_eq!(
            wheel_error("pkg-1.0-py3-none-any.whl", &bytes),
            "pypi_deploy: wheel pkg-1.0-py3-none-any.whl ships build-only file pkg/include/pkg.h"
        );
    }

    #[test]
    fn pypi_rejects_non_zip_wheel() {
        let error = wheel_error("pkg-1.0-py3-none-any.whl", b"not a zip");
        assert!(error.contains("is not a zip file"), "{error}");
    }

    #[test]
    fn pypi_wheelhouse_builds_index() {
        let scratch = scratch_dir();
        let wheel = scratch.path().join("pypi_demo-0.0.0-py3-none-any.whl");
        std::fs::write(&wheel, wheel_bytes(&[])).expect("write wheel");
        let sdist = scratch.path().join("pypi_demo-0.0.0.tar.gz");
        std::fs::write(&sdist, b"pypi-demo-sdist-bytes-v1").expect("write sdist");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let house =
            pypi_build_wheelhouse(&wheel, Some(&sdist), &outdir, "pypi_demo").expect("house");
        assert_eq!(house, outdir.join("pypi_demo-wheelhouse"));
        for src in [&wheel, &sdist] {
            let base = basename(src).expect("base");
            let dest = house.join(&base);
            assert!(dest.is_file());
            assert_eq!(
                dx_digest::sha256_file_hex(&dest).expect("hex"),
                dx_digest::sha256_file_hex(src).expect("hex")
            );
        }
        let index = house
            .join("simple-index")
            .join("pypi-demo")
            .join("index.html");
        let body = std::fs::read_to_string(&index).expect("index");
        assert!(body.contains("pypi_demo-0.0.0-py3-none-any.whl"));
        assert!(body.contains("pypi_demo-0.0.0.tar.gz"));
        assert!(body.contains("#sha256="));
        let house_only =
            pypi_build_wheelhouse(&wheel, None, &outdir, "pypi_demo").expect("wheel only");
        assert!(house_only.join(wheel.file_name().expect("base")).is_file());
    }

    #[test]
    fn pypi_index_uses_normalized_distribution_identity() {
        assert_eq!(pypi_normalize_name("Foo.Bar__baz--Qux"), "foo-bar-baz-qux");
        assert_eq!(
            pypi_artifact_identity("My_Pkg.Name-1.0-py3-none-any.whl").expect("wheel"),
            ("my-pkg-name".to_owned(), "1.0".to_owned())
        );
        assert_eq!(
            pypi_artifact_identity("My_Pkg.Name-1.0.tar.gz").expect("sdist"),
            ("my-pkg-name".to_owned(), "1.0".to_owned())
        );
        assert!(pypi_artifact_identity("broken.whl").is_err());
        assert!(pypi_artifact_identity("pkg.zip").is_err());
        let scratch = scratch_dir();
        let wheel = scratch.path().join("My_Pkg.Name-1.0-py3-none-any.whl");
        std::fs::write(&wheel, wheel_bytes(&[])).expect("wheel");
        let sdist = scratch.path().join("my-pkg-name-1.0.tar.gz");
        std::fs::write(&sdist, b"s").expect("sdist");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let house = pypi_build_wheelhouse(&wheel, Some(&sdist), &outdir, "publish").expect("house");
        assert_eq!(house, outdir.join("publish-wheelhouse"));
        assert!(house
            .join("simple-index")
            .join("my-pkg-name")
            .join("index.html")
            .is_file());
        assert!(!house.join("simple-index").join("publish").exists());
        let other = scratch.path().join("other-1.0.tar.gz");
        std::fs::write(&other, b"o").expect("other");
        let error =
            pypi_build_wheelhouse(&wheel, Some(&other), &outdir, "publish").expect_err("conflict");
        assert!(error.to_string().contains("my-pkg-name 1.0"), "{error}");
        let newer = scratch.path().join("my_pkg_name-2.0.tar.gz");
        std::fs::write(&newer, b"n").expect("newer");
        assert!(pypi_build_wheelhouse(&wheel, Some(&newer), &outdir, "publish").is_err());
    }

    #[cfg(unix)]
    fn fake_twine(dir: &Path, body: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("twine");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("twine");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    }

    #[cfg(unix)]
    fn upload_fixture(dir: &Path, sdist_name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let wheel = dir.join("pkg-1.0-py3-none-any.whl");
        std::fs::write(&wheel, wheel_bytes(&[])).expect("wheel");
        let sdist = dir.join(sdist_name);
        std::fs::write(&sdist, b"s").expect("sdist");
        (wheel, sdist)
    }

    #[test]
    #[cfg(unix)]
    fn pypi_live_upload_refuses_mismatch_and_uploads_copies() {
        let scratch = scratch_dir();
        let dir = scratch.path();
        let argv = dir.join("argv");
        let twine = fake_twine(dir, &format!("echo \"$@\" >> {}", argv.display()));
        let stage = dir.join("stage");
        let (wheel, bad) = upload_fixture(dir, "pkg-2.0.tar.gz");
        assert_eq!(
            pypi_upload(&twine, &wheel, Some(&bad), &stage, "https://u", "tok"),
            1
        );
        assert!(!argv.exists());
        let good = dir.join("pkg-1.0.tar.gz");
        std::fs::write(&good, b"s").expect("sdist");
        assert_eq!(
            pypi_upload(&twine, &wheel, Some(&good), &stage, "https://u", "tok"),
            0
        );
        let lines = std::fs::read_to_string(&argv).expect("argv");
        let lines: Vec<&str> = lines.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("check --strict "), "{}", lines[0]);
        assert!(
            lines[1].starts_with("upload --non-interactive "),
            "{}",
            lines[1]
        );
        for line in &lines {
            assert!(line.contains(&stage.display().to_string()), "{line}");
            assert!(!line.contains(&wheel.display().to_string()), "{line}");
        }
        assert!(!stage.exists());
    }

    #[test]
    #[cfg(unix)]
    fn pypi_live_upload_stops_on_failed_check() {
        let scratch = scratch_dir();
        let dir = scratch.path();
        let marker = dir.join("uploaded");
        let body = format!("[ \"$1\" = check ] && exit 1\ntouch {}", marker.display());
        let twine = fake_twine(dir, &body);
        let (wheel, sdist) = upload_fixture(dir, "pkg-1.0.tar.gz");
        let stage = dir.join("stage");
        let code = pypi_upload(&twine, &wheel, Some(&sdist), &stage, "https://u", "tok");
        assert_eq!(code, 1);
        assert!(!marker.exists());
    }

    #[test]
    fn pypi_duplicate_version_is_named() {
        let note = pypi_duplicate_note(
            "HTTPError: File Already Exists",
            "pkg.whl",
            "1.0",
            "https://u",
        );
        assert_eq!(
            note.as_deref(),
            Some("pypi_deploy: pkg.whl version 1.0 is already on https://u; upload a new version")
        );
        assert!(pypi_duplicate_note("boom", "pkg.whl", "1.0", "https://u").is_none());
        assert_eq!(pypi_scrub(b"a tok b", "tok"), "a [redacted] b");
    }
}
