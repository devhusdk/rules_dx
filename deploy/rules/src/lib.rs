#![cfg_attr(not(test), deny(clippy::expect_used, clippy::unwrap_used))]

use std::io::{self, Write};
use std::path::Path;

const BLOCK: usize = 512;
const RECORDSIZE: usize = 20 * BLOCK;
const MTIME: u64 = 0;
const UID: u64 = 0;
const GID: u64 = 0;

fn octal_field(value: u64, digits: usize) -> io::Result<Vec<u8>> {
    let max = 8u64.pow((digits - 1) as u32);
    if value >= max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("value {value} overflows {digits}-byte octal field"),
        ));
    }
    let text = format!("{:0>width$o}", value, width = digits - 1);
    let mut out = text.into_bytes();
    out.push(0);
    Ok(out)
}

fn tar_header(name: &str, size: u64, mode: u32) -> io::Result<[u8; BLOCK]> {
    let name_bytes = name.as_bytes();
    if name_bytes.is_empty() || name_bytes.len() > 100 || name_bytes.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("tar member name {name:?} must be 1-100 bytes with no NUL"),
        ));
    }
    if name_bytes.contains(&b'/') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("tar member name {name:?} must be a basename with no slash"),
        ));
    }
    let mut header = [0u8; BLOCK];
    header[0..name_bytes.len()].copy_from_slice(name_bytes);
    let mode_field = octal_field(u64::from(mode & 0o7777), 8)?;
    header[100..108].copy_from_slice(&mode_field);
    let uid_field = octal_field(UID, 8)?;
    header[108..116].copy_from_slice(&uid_field);
    let gid_field = octal_field(GID, 8)?;
    header[116..124].copy_from_slice(&gid_field);
    let size_field = octal_field(size, 12)?;
    header[124..136].copy_from_slice(&size_field);
    let mtime_field = octal_field(MTIME, 12)?;
    header[136..148].copy_from_slice(&mtime_field);
    for slot in header.iter_mut().take(156).skip(148) {
        *slot = b' ';
    }
    header[156] = b'0';
    header[257..265].copy_from_slice(b"ustar\x0000");
    let mut sum: u32 = 0;
    for byte in header {
        sum += u32::from(byte);
    }
    let checksum_text = format!("{sum:06o}\0 ");
    let checksum_bytes = checksum_text.as_bytes();
    header[148..156].copy_from_slice(checksum_bytes);
    Ok(header)
}

fn tar_image(name: &str, data: &[u8], mode: u32) -> io::Result<Vec<u8>> {
    let header = tar_header(name, data.len() as u64, mode)?;
    let data_blocks = data.len().div_ceil(BLOCK) * BLOCK;
    let used = BLOCK + data_blocks + 2 * BLOCK;
    let padded = used.div_ceil(RECORDSIZE) * RECORDSIZE;
    let mut out = Vec::with_capacity(padded);
    out.extend_from_slice(&header);
    out.extend_from_slice(data);
    out.resize(out.len() + (data_blocks - data.len()), 0);
    out.resize(out.len() + 2 * BLOCK, 0);
    out.resize(padded, 0);
    Ok(out)
}

fn gzip_compress(tar: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = flate2::GzBuilder::new()
        .mtime(0)
        .operating_system(255)
        .write(Vec::new(), flate2::Compression::best());
    encoder.write_all(tar)?;
    encoder.finish()
}

fn basename(path: &Path) -> io::Result<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("path {} has no UTF-8 basename", path.display()),
            )
        })
}

fn member_mode(path: &Path) -> io::Result<u32> {
    let metadata = std::fs::metadata(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let executable = metadata.permissions().mode() & 0o111 != 0;
        Ok(if executable { 0o755 } else { 0o644 })
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        Ok(0o644)
    }
}

pub fn archive_file(src: &Path, dst: &Path) -> io::Result<()> {
    let data = std::fs::read(src)?;
    let name = basename(src)?;
    let mode = member_mode(src)?;
    let tar = tar_image(&name, &data, mode)?;
    let gz = gzip_compress(&tar)?;
    std::fs::write(dst, gz)?;
    Ok(())
}

pub fn archive_bytes(data: &[u8], name: &str, executable: bool) -> io::Result<Vec<u8>> {
    let mode = if executable { 0o755 } else { 0o644 };
    let tar = tar_image(name, data, mode)?;
    gzip_compress(&tar)
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest as _;
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

pub fn hash_file(src: &Path, dst: &Path) -> io::Result<()> {
    let base = basename(src)?;
    let mut hasher = sha2::Sha256::new();
    use sha2::Digest as _;
    let mut file = std::fs::File::open(src)?;
    let mut buf = vec![0u8; 64 << 10];
    loop {
        use std::io::Read as _;
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    let line = format!("{}  {base}\n", hex::encode(hasher.finalize()));
    std::fs::write(dst, line.as_bytes())?;
    Ok(())
}

pub fn hash_line(data: &[u8], basename: &str) -> String {
    format!("{}  {basename}\n", sha256_hex(data))
}

pub fn bin_usage(prog: &str, usage: &str) -> i32 {
    eprintln!("usage: {prog} {usage}");
    1
}

pub fn bin_cannot(prog: &str, action: &str, target: &Path, error: impl std::fmt::Display) -> i32 {
    eprintln!("{prog}: cannot {action} {}: {error}", target.display());
    1
}

/// Shared deploy-launcher helpers.
pub fn sha256_file_hex(path: &Path) -> io::Result<String> {
    use sha2::Digest as _;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = vec![0u8; 64 << 10];
    loop {
        use std::io::Read as _;
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Copies one file and verifies byte identity.
pub fn copy_verified(src: &Path, dest: &Path) -> io::Result<String> {
    std::fs::copy(src, dest)?;
    let want = sha256_file_hex(src)?;
    let got = sha256_file_hex(dest)?;
    if want != got {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "byte mismatch for {} (expected sha256 {want}, got {got})",
                src.display(),
            ),
        ));
    }
    Ok(want)
}

/// Resolves one runfiles rlocation without external deps.
pub fn resolve_runfile(rloc: &str) -> io::Result<std::path::PathBuf> {
    if rloc.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "empty runfiles rlocation",
        ));
    }
    if let Ok(dir) = std::env::var("RUNFILES_DIR") {
        let candidate = std::path::Path::new(&dir).join(rloc);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    if let Ok(manifest) = std::env::var("RUNFILES_MANIFEST_FILE") {
        if let Ok(text) = std::fs::read_to_string(&manifest) {
            for line in text.lines() {
                let mut parts = line.splitn(2, ' ');
                if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                    if key == rloc {
                        let candidate = std::path::PathBuf::from(value);
                        if candidate.exists() {
                            return Ok(candidate);
                        }
                    }
                }
            }
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        for candidate in [
            exe.with_extension("runfiles").join(rloc),
            exe.with_extension("exe.runfiles").join(rloc),
        ] {
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("runfile not found for '{rloc}'"),
    ))
}

/// Returns the deploy output directory.
pub fn deploy_outdir(argv: &[String]) -> std::path::PathBuf {
    if argv.len() >= 2 {
        return std::path::PathBuf::from(&argv[1]);
    }
    std::env::var("BUILD_WORKSPACE_DIRECTORY")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        })
}

/// Filters process env to an allowlist plus extras.
pub fn minimal_env(
    keep: &[&str],
    extra: &[(&str, &str)],
) -> std::collections::HashMap<String, String> {
    let mut env = std::collections::HashMap::new();
    for key in keep {
        if let Ok(value) = std::env::var(key) {
            env.insert((*key).to_owned(), value);
        }
    }
    for (key, value) in extra {
        env.insert((*key).to_owned(), (*value).to_owned());
    }
    env
}

/// Escapes one JSON string value.
pub fn json_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Escapes one XML text value.
pub fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Writes one owner-only file.
pub fn write_secure_file(
    directory: &Path,
    name: &str,
    content: &str,
) -> io::Result<std::path::PathBuf> {
    let path = directory.join(name);
    std::fs::write(&path, content.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(path)
}

/// Stages one release tarball pair.
pub fn archive_stage_release(
    app_src: &Path,
    tarball_src: &Path,
    checksum_src: &Path,
    outdir: &Path,
) -> io::Result<(String, std::path::PathBuf, std::path::PathBuf)> {
    let app_name = basename(app_src)?;
    let text = std::fs::read_to_string(checksum_src)?;
    let expected = text
        .split_whitespace()
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "empty checksum file"))?;
    let actual = sha256_file_hex(tarball_src)?;
    if expected != actual {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "archive_deploy: checksum mismatch for {} (expected {expected}, actual {actual})",
                tarball_src.display(),
            ),
        ));
    }
    std::fs::create_dir_all(outdir)?;
    let tarball_base = basename(tarball_src)?;
    let checksum_base = basename(checksum_src)?;
    let tarball_dest = outdir.join(&tarball_base);
    let checksum_dest = outdir.join(&checksum_base);
    std::fs::copy(tarball_src, &tarball_dest)?;
    std::fs::copy(checksum_src, &checksum_dest)?;
    Ok((app_name, tarball_dest, checksum_dest))
}

/// Runs the archive deploy launcher.
pub fn archive_main(
    app_rloc: &str,
    tarball_rloc: &str,
    checksum_rloc: &str,
    argv: &[String],
) -> i32 {
    if argv.len() > 2 {
        eprintln!(
            "archive_deploy: this deploy target takes at most an output directory; the release is exactly the files pinned at analysis time"
        );
        return 1;
    }
    let outdir = deploy_outdir(argv);
    if std::fs::create_dir_all(&outdir).is_err() {
        eprintln!("archive_deploy: cannot create {}", outdir.display());
        return 1;
    }
    let resolved = (|| -> io::Result<(String, std::path::PathBuf, std::path::PathBuf)> {
        let app = resolve_runfile(app_rloc)?;
        let tarball = resolve_runfile(tarball_rloc)?;
        let checksum = resolve_runfile(checksum_rloc)?;
        archive_stage_release(&app, &tarball, &checksum, &outdir)
    })();
    match resolved {
        Ok((app_name, tarball_dest, checksum_dest)) => {
            let profile = std::env::var("DX_PROFILE").unwrap_or_else(|_| "<unset>".to_owned());
            println!("archive_deploy: released {app_name} (profile: {profile})");
            println!("  tarball:  {}", tarball_dest.display());
            println!("  checksum: {}", checksum_dest.display());
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

/// Builds one PyPI wheelhouse.
pub fn pypi_build_wheelhouse(
    wheel_src: &Path,
    sdist_src: Option<&Path>,
    outdir: &Path,
    dist_name: &str,
) -> io::Result<std::path::PathBuf> {
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
    let index_dir = house.join("simple-index").join(dist_name);
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

/// Runs the PyPI deploy launcher.
pub fn pypi_main(
    wheel_rloc: &str,
    sdist_rloc: &str,
    dist_name: &str,
    repository_url: &str,
    argv: &[String],
) -> i32 {
    if argv.len() > 2 {
        eprintln!(
            "pypi_deploy: this deploy target takes at most an output directory; the wheelhouse is exactly the files pinned at analysis time"
        );
        return 1;
    }
    let outdir = deploy_outdir(argv);
    if std::fs::create_dir_all(&outdir).is_err() {
        eprintln!("pypi_deploy: cannot create {}", outdir.display());
        return 1;
    }
    let wheel = match resolve_runfile(wheel_rloc) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("pypi_deploy: {error}");
            return 1;
        }
    };
    let sdist = if sdist_rloc.is_empty() {
        None
    } else {
        match resolve_runfile(sdist_rloc) {
            Ok(path) => Some(path),
            Err(error) => {
                eprintln!("pypi_deploy: {error}");
                return 1;
            }
        }
    };
    if std::env::var("PYPI_PUBLISH_LIVE").unwrap_or_default() == "1" {
        let token = std::env::var("PYPI_API_TOKEN").unwrap_or_default();
        let approved = std::env::var("PYPI_PUBLISH_APPROVED").unwrap_or_default();
        if token.is_empty() {
            eprintln!(
                "pypi_deploy: live upload needs PYPI_API_TOKEN plus explicit owner approval (PYPI_PUBLISH_APPROVED=1); refusing"
            );
            return 1;
        }
        if approved != "1" {
            eprintln!(
                "pypi_deploy: live upload needs PYPI_PUBLISH_APPROVED=1 (owner approval); refusing"
            );
            return 1;
        }
        let mut files = vec![wheel.clone()];
        if let Some(sdist) = &sdist {
            files.push(sdist.clone());
        }
        let keep = [
            "HOME",
            "LANG",
            "LC_ALL",
            "PATH",
            "TMPDIR",
            "USER",
            "LOGNAME",
            "SystemRoot",
            "SystemDrive",
            "PATHEXT",
        ];
        let env = minimal_env(
            &keep,
            &[("TWINE_USERNAME", "__token__"), ("TWINE_PASSWORD", &token)],
        );
        let mut cmd = std::process::Command::new("twine");
        cmd.arg("upload")
            .arg("--non-interactive")
            .arg("--repository-url")
            .arg(repository_url);
        for file in &files {
            cmd.arg(file);
        }
        cmd.env_clear();
        for (key, value) in &env {
            cmd.env(key, value);
        }
        match cmd.status() {
            Ok(status) if status.success() => {
                let base = wheel.file_name().and_then(|n| n.to_str()).unwrap_or("");
                println!("pypi_deploy: uploaded {base} to {repository_url}");
                return 0;
            }
            Ok(status) => {
                eprintln!("pypi_deploy: twine failed with {status}");
                return 1;
            }
            Err(error) => {
                eprintln!("pypi_deploy: {error}");
                return 1;
            }
        }
    }
    match pypi_build_wheelhouse(&wheel, sdist.as_deref(), &outdir, dist_name) {
        Ok(house) => {
            let profile = std::env::var("DX_PROFILE").unwrap_or_else(|_| "<unset>".to_owned());
            println!("pypi_deploy: staged {dist_name} (profile: {profile})");
            println!("  wheelhouse: {}", house.display());
            println!(
                "  index: {}",
                house
                    .join("simple-index")
                    .join(dist_name)
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

/// Builds one NuGet folder feed.
pub fn nuget_build_feed(
    nupkg_src: &Path,
    outdir: &Path,
    package_id: &str,
) -> io::Result<std::path::PathBuf> {
    let feed = outdir.join(package_id.to_owned() + "-feed");
    std::fs::create_dir_all(&feed)?;
    let base = basename(nupkg_src)?;
    let dest = feed.join(&base);
    copy_verified(nupkg_src, &dest)?;
    Ok(feed)
}

/// Runs the NuGet deploy launcher.
pub fn nuget_main(
    nupkg_rloc: &str,
    package_id: &str,
    package_version: &str,
    package_source: &str,
    argv: &[String],
) -> i32 {
    if argv.len() > 2 {
        eprintln!(
            "nuget_deploy: this deploy target takes at most an output directory; the feed is exactly the file pinned at analysis time"
        );
        return 1;
    }
    let outdir = deploy_outdir(argv);
    if std::fs::create_dir_all(&outdir).is_err() {
        eprintln!("nuget_deploy: cannot create {}", outdir.display());
        return 1;
    }
    let nupkg = match resolve_runfile(nupkg_rloc) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("nuget_deploy: {error}");
            return 1;
        }
    };
    if std::env::var("NUGET_PUBLISH_LIVE").unwrap_or_default() == "1" {
        let token = std::env::var("NUGET_API_KEY").unwrap_or_default();
        let approved = std::env::var("NUGET_PUBLISH_APPROVED").unwrap_or_default();
        if package_version == "0.0.0" {
            eprintln!("nuget_deploy: live push refuses version 0.0.0; set a real version");
            return 1;
        }
        if token.is_empty() {
            eprintln!(
                "nuget_deploy: live push needs NUGET_API_KEY plus explicit owner approval (NUGET_PUBLISH_APPROVED=1); refusing"
            );
            return 1;
        }
        if approved != "1" {
            eprintln!(
                "nuget_deploy: live push needs NUGET_PUBLISH_APPROVED=1 (owner approval); refusing"
            );
            return 1;
        }
        let status = std::process::Command::new("dotnet")
            .arg("nuget")
            .arg("push")
            .arg(&nupkg)
            .arg("--source")
            .arg(package_source)
            .arg("--api-key")
            .arg(&token)
            .arg("--skip-duplicate")
            .status();
        match status {
            Ok(status) if status.success() => {
                println!("nuget_deploy: pushed {package_id} {package_version}");
                return 0;
            }
            Ok(status) => {
                eprintln!("nuget_deploy: dotnet failed with {status}");
                return 1;
            }
            Err(error) => {
                eprintln!("nuget_deploy: {error}");
                return 1;
            }
        }
    }
    match nuget_build_feed(&nupkg, &outdir, package_id) {
        Ok(feed) => {
            let profile = std::env::var("DX_PROFILE").unwrap_or_else(|_| "<unset>".to_owned());
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

/// Builds one crates vendor tree.
pub fn crates_build_vendor(
    crate_files: &[std::path::PathBuf],
    outdir: &Path,
    crate_name: &str,
    version: &str,
) -> io::Result<std::path::PathBuf> {
    if crate_files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "crates vendor: need at least one crate source file",
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for src in crate_files {
        let base = basename(src)?;
        if base.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("crates vendor: empty basename for '{}'", src.display()),
            ));
        }
        if !seen.insert(base.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("crates vendor: duplicate basename '{base}' (keep basenames unique)"),
            ));
        }
    }
    let house = outdir.join(crate_name.to_owned() + "-vendor");
    let vendor_dir = house.join("vendor").join(crate_name);
    let registry_dir = house.join("registry").join(crate_name).join(version);
    std::fs::create_dir_all(&vendor_dir)?;
    std::fs::create_dir_all(&registry_dir)?;
    let mut sorted: Vec<&std::path::PathBuf> = crate_files.iter().collect();
    sorted.sort();
    let mut entries: Vec<(String, String)> = Vec::new();
    for src in sorted {
        let base = basename(src)?;
        let want = sha256_file_hex(src)?;
        for dest_dir in [&vendor_dir, &registry_dir] {
            let dest = dest_dir.join(&base);
            std::fs::copy(src, &dest)?;
            let got = sha256_file_hex(&dest)?;
            if want != got {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "crates vendor: byte mismatch for {base} (expected sha256 {want}, got {got})"
                    ),
                ));
            }
        }
        entries.push((base, want));
    }
    entries.sort();
    let package_digest = {
        use sha2::Digest as _;
        let mut hasher = sha2::Sha256::new();
        hasher.update(
            entries
                .iter()
                .map(|(base, digest)| format!("{base}:{digest}"))
                .collect::<Vec<_>>()
                .join("\n")
                .as_bytes(),
        );
        hex::encode(hasher.finalize())
    };
    let mut checksum_body = String::from("{\n  \"files\": {\n");
    for (index, (base, digest)) in entries.iter().enumerate() {
        checksum_body.push_str(&format!(
            "    \"{}\": \"{}\"{}",
            json_escape(base),
            digest,
            if index + 1 < entries.len() {
                ",\n"
            } else {
                "\n"
            },
        ));
    }
    checksum_body.push_str(&format!("  }},\n  \"package\": \"{package_digest}\"\n}}\n"));
    std::fs::write(
        vendor_dir.join(".cargo-checksum.json"),
        checksum_body.as_bytes(),
    )?;
    let mut index_body = String::from("{\n  \"files\": [\n");
    for (index, (base, digest)) in entries.iter().enumerate() {
        index_body.push_str(&format!(
            "    {{\"name\": \"{}\", \"sha256\": \"{}\"}}{}",
            json_escape(base),
            digest,
            if index + 1 < entries.len() {
                ",\n"
            } else {
                "\n"
            },
        ));
    }
    index_body.push_str(&format!(
        "  ],\n  \"name\": \"{}\",\n  \"version\": \"{}\"\n}}\n",
        json_escape(crate_name),
        json_escape(version),
    ));
    std::fs::write(registry_dir.join("index.json"), index_body.as_bytes())?;
    Ok(house)
}

/// Runs the crates deploy launcher.
pub fn crates_main(
    crate_rlocs: &str,
    crate_name: &str,
    version: &str,
    allow_dirty: bool,
    argv: &[String],
) -> i32 {
    if argv.len() > 2 {
        eprintln!(
            "crates_deploy: this deploy target takes at most an output directory; the vendor tree is exactly the files pinned at analysis time"
        );
        return 1;
    }
    let outdir = deploy_outdir(argv);
    if std::fs::create_dir_all(&outdir).is_err() {
        eprintln!("crates_deploy: cannot create {}", outdir.display());
        return 1;
    }
    let mut files = Vec::new();
    for rloc in crate_rlocs.split(';').filter(|s| !s.is_empty()) {
        match resolve_runfile(rloc) {
            Ok(path) => files.push(path),
            Err(error) => {
                eprintln!("crates_deploy: {error}");
                return 1;
            }
        }
    }
    if std::env::var("CRATES_PUBLISH_LIVE").unwrap_or_default() == "1" {
        if std::env::var("CRATES_ALLOW_DIRTY").unwrap_or_default() == "1" || allow_dirty {
            eprintln!(
                "crates_deploy: --allow-dirty rejected by default; publish from a clean tree instead"
            );
            return 1;
        }
        let token = std::env::var("CARGO_REGISTRY_TOKEN").unwrap_or_default();
        let approved = std::env::var("CRATES_PUBLISH_APPROVED").unwrap_or_default();
        if version == "0.0.0" {
            eprintln!("crates_deploy: live publish refuses version 0.0.0; set a real version");
            return 1;
        }
        if token.is_empty() {
            eprintln!(
                "crates_deploy: live publish needs CARGO_REGISTRY_TOKEN plus explicit owner approval (CRATES_PUBLISH_APPROVED=1); refusing"
            );
            return 1;
        }
        if approved != "1" {
            eprintln!(
                "crates_deploy: live publish needs CRATES_PUBLISH_APPROVED=1 (owner approval); refusing"
            );
            return 1;
        }
        let keep = [
            "HOME",
            "LANG",
            "LC_ALL",
            "PATH",
            "TMPDIR",
            "USER",
            "LOGNAME",
            "SystemRoot",
            "SystemDrive",
            "PATHEXT",
            "CARGO_HOME",
            "RUSTUP_HOME",
        ];
        let env = minimal_env(&keep, &[("CARGO_REGISTRY_TOKEN", &token)]);
        let staging = match tempfile_stage_dir("crates-publish-") {
            Ok(dir) => dir,
            Err(error) => {
                eprintln!("crates_deploy: {error}");
                return 1;
            }
        };
        for src in &files {
            if let Ok(base) = basename(src) {
                let dest = staging.join(&base);
                if std::fs::copy(src, &dest).is_err() {
                    eprintln!("crates_deploy: cannot stage {}", src.display());
                    return 1;
                }
            }
        }
        let manifest = staging.join("Cargo.toml");
        if !manifest.is_file() {
            eprintln!("crates_deploy: staged crate is missing Cargo.toml; refusing");
            return 1;
        }
        let mut cmd = std::process::Command::new("cargo");
        cmd.arg("publish").arg("--manifest-path").arg(&manifest);
        cmd.env_clear();
        for (key, value) in &env {
            cmd.env(key, value);
        }
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
    match crates_build_vendor(&files, &outdir, crate_name, version) {
        Ok(house) => {
            let profile = std::env::var("DX_PROFILE").unwrap_or_else(|_| "<unset>".to_owned());
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

fn tempfile_stage_dir(prefix: &str) -> io::Result<std::path::PathBuf> {
    let mut dir = std::env::temp_dir().join(format!(
        "{prefix}{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let mut counter = 0;
    while dir.exists() {
        counter += 1;
        dir = std::env::temp_dir().join(format!(
            "{prefix}{}{counter}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        if counter > 100 {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "cannot stage tempdir",
            ));
        }
    }
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Formats one GitHub release command.
pub fn github_release_command(tag: &str, asset_bases: &[String]) -> String {
    let mut cmd = format!("gh release create {tag}");
    for base in asset_bases {
        cmd.push_str(&format!(" {base}"));
    }
    cmd.push_str(" --draft --verify-tag");
    cmd
}

/// Builds one GitHub release staging dir.
pub fn github_build_staging(
    asset_srcs: &[std::path::PathBuf],
    outdir: &Path,
    deploy_name: &str,
    tag: &str,
) -> io::Result<std::path::PathBuf> {
    let staging = outdir.join(deploy_name.to_owned() + "-release");
    std::fs::create_dir_all(&staging)?;
    let mut sorted: Vec<&std::path::PathBuf> = asset_srcs.iter().collect();
    sorted.sort();
    let mut bases = Vec::new();
    let mut lines = vec![String::from("# would-run manifest for github_deploy")];
    for src in sorted {
        let base = basename(src)?;
        let dest = staging.join(&base);
        let digest = copy_verified(src, &dest)?;
        bases.push(base.clone());
        lines.push(format!("asset-sha256: {digest}  {base}"));
    }
    lines.push(github_release_command(tag, &bases));
    std::fs::write(staging.join("would-run.txt"), lines.join("\n").as_bytes())?;
    Ok(staging)
}

/// Runs the GitHub deploy launcher.
pub fn github_main(asset_rlocs: &str, deploy_name: &str, tag: &str, argv: &[String]) -> i32 {
    if argv.len() > 2 {
        eprintln!(
            "github_deploy: this deploy target takes at most an output directory; the release is exactly the artifacts pinned at analysis time"
        );
        return 1;
    }
    let outdir = deploy_outdir(argv);
    if std::fs::create_dir_all(&outdir).is_err() {
        eprintln!("github_deploy: cannot create {}", outdir.display());
        return 1;
    }
    let mut assets = Vec::new();
    for rloc in asset_rlocs.split(';').filter(|s| !s.is_empty()) {
        match resolve_runfile(rloc) {
            Ok(path) => assets.push(path),
            Err(error) => {
                eprintln!("github_deploy: {error}");
                return 1;
            }
        }
    }
    if std::env::var("GH_RELEASE_LIVE").unwrap_or_default() == "1" {
        let approved = std::env::var("GH_RELEASE_APPROVED").unwrap_or_default();
        if tag == "v0.0.0-dryrun" {
            eprintln!(
                "github_deploy: live publish refuses placeholder tag v0.0.0-dryrun; push a real tag first"
            );
            return 1;
        }
        if approved != "1" {
            eprintln!(
                "github_deploy: live publish needs GH_RELEASE_APPROVED=1 (owner approval); refusing"
            );
            return 1;
        }
        if which_on_path("gh").is_none() {
            eprintln!("github_deploy: 'gh' CLI not found on PATH");
            return 1;
        }
        let mut cmd = std::process::Command::new("gh");
        cmd.arg("release").arg("create").arg(tag);
        for src in &assets {
            cmd.arg(src);
        }
        cmd.arg("--draft").arg("--verify-tag");
        match cmd.status() {
            Ok(status) if status.success() => {
                println!("github_deploy: published draft {tag}");
                return 0;
            }
            Ok(status) => {
                eprintln!("github_deploy: gh failed with {status}");
                return 1;
            }
            Err(error) => {
                eprintln!("github_deploy: {error}");
                return 1;
            }
        }
    }
    match github_build_staging(&assets, &outdir, deploy_name, tag) {
        Ok(staging) => {
            let mut sorted = assets.clone();
            sorted.sort();
            if std::env::var("GH_RELEASE_DRY_RUN").unwrap_or_default() == "1" {
                println!("github_deploy: dry run (GH_RELEASE_DRY_RUN=1); would create a draft release, publishing nothing:");
            } else {
                println!("github_deploy: staged draft release, publishing nothing:");
            }
            println!("  tag: {tag}");
            for src in &sorted {
                if let Ok(base) = basename(src) {
                    println!("  asset: {base} ({})", src.display());
                }
            }
            let bases: Vec<String> = sorted.iter().filter_map(|src| basename(src).ok()).collect();
            println!("  command: {}", github_release_command(tag, &bases));
            println!("  staging: {}", staging.display());
            println!("  manifest: {}", staging.join("would-run.txt").display());
            0
        }
        Err(error) => {
            eprintln!("github_deploy: {error}");
            1
        }
    }
}

fn which_on_path(tool: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var("PATH").unwrap_or_default();
    #[cfg(windows)]
    let sep = ';';
    #[cfg(not(windows))]
    let sep = ':';
    for dir in path.split(sep) {
        if dir.is_empty() {
            continue;
        }
        let candidate = std::path::Path::new(dir).join(tool);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let exe = std::path::Path::new(dir).join(tool.to_owned() + ".exe");
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

/// Maps one Maven group to a path.
pub fn maven_group_path(group: &str) -> String {
    group.replace('.', "/")
}

/// Builds one Maven file repo.
pub fn maven_build_file_repo(
    jar_src: &Path,
    pom_src: &Path,
    outdir: &Path,
    group: &str,
    artifact: &str,
    version: &str,
) -> io::Result<std::path::PathBuf> {
    if group.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "maven file repo: need a non-empty groupId",
        ));
    }
    if artifact.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "maven file repo: need a non-empty artifactId",
        ));
    }
    if version.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "maven file repo: need a non-empty version",
        ));
    }
    let house = outdir.join(artifact.to_owned() + "-repo");
    let dest_dir = house
        .join(maven_group_path(group))
        .join(artifact)
        .join(version);
    std::fs::create_dir_all(&dest_dir)?;
    let jar_base = format!("{artifact}-{version}.jar");
    let pom_base = format!("{artifact}-{version}.pom");
    for (src, base) in [(jar_src, jar_base.clone()), (pom_src, pom_base.clone())] {
        let dest = dest_dir.join(&base);
        let digest = copy_verified(src, &dest)?;
        std::fs::write(
            dest_dir.join(base.clone() + ".sha256"),
            format!("{digest}  {base}\n").as_bytes(),
        )?;
    }
    let metadata_dir = house.join(maven_group_path(group)).join(artifact);
    std::fs::create_dir_all(&metadata_dir)?;
    let metadata = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<metadata>\n  <groupId>{group}</groupId>\n  <artifactId>{artifact}</artifactId>\n  <version>{version}</version>\n  <versioning>\n    <latest>{version}</latest>\n    <release>{version}</release>\n    <versions>\n      <version>{version}</version>\n    </versions>\n  </versioning>\n</metadata>\n"
    );
    std::fs::write(metadata_dir.join("maven-metadata.xml"), metadata.as_bytes())?;
    Ok(house)
}

/// Renders one Maven settings.xml.
pub fn maven_settings_xml(username: &str, password: &str, passphrase: &str) -> String {
    let mut body = format!(
        "<settings>\n  <servers>\n    <server>\n      <id>dx-staging</id>\n      <username>{}</username>\n      <password>{}</password>\n    </server>\n  </servers>\n",
        xml_escape(username),
        xml_escape(password),
    );
    if !passphrase.is_empty() {
        body.push_str(&format!(
            "  <profiles>\n    <profile>\n      <id>dx-gpg-passphrase</id>\n      <properties>\n        <gpg.passphrase>{}</gpg.passphrase>\n      </properties>\n    </profile>\n  </profiles>\n  <activeProfiles>\n    <activeProfile>dx-gpg-passphrase</activeProfile>\n  </activeProfiles>\n",
            xml_escape(passphrase),
        ));
    }
    body.push_str("</settings>\n");
    body
}

/// Runs the Maven deploy launcher.
pub fn maven_main(
    jar_rloc: &str,
    pom_rloc: &str,
    group: &str,
    artifact: &str,
    version: &str,
    repository_url: &str,
    argv: &[String],
) -> i32 {
    if argv.len() > 2 {
        eprintln!(
            "maven_deploy: this deploy target takes at most an output directory; the file repo is exactly the files pinned at analysis time"
        );
        return 1;
    }
    let outdir = deploy_outdir(argv);
    if std::fs::create_dir_all(&outdir).is_err() {
        eprintln!("maven_deploy: cannot create {}", outdir.display());
        return 1;
    }
    let jar = match resolve_runfile(jar_rloc) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("maven_deploy: {error}");
            return 1;
        }
    };
    let pom = match resolve_runfile(pom_rloc) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("maven_deploy: {error}");
            return 1;
        }
    };
    if std::env::var("MAVEN_PUBLISH_LIVE").unwrap_or_default() == "1" {
        let username = std::env::var("MAVEN_USERNAME").unwrap_or_default();
        let password = std::env::var("MAVEN_PASSWORD").unwrap_or_default();
        let approved = std::env::var("MAVEN_PUBLISH_APPROVED").unwrap_or_default();
        if version == "0.0.0" {
            eprintln!("maven_deploy: live staging refuses version 0.0.0; set a real version");
            return 1;
        }
        if username.is_empty() || password.is_empty() {
            eprintln!(
                "maven_deploy: live staging needs MAVEN_USERNAME plus MAVEN_PASSWORD with explicit owner approval (MAVEN_PUBLISH_APPROVED=1); refusing"
            );
            return 1;
        }
        if approved != "1" {
            eprintln!(
                "maven_deploy: live staging needs MAVEN_PUBLISH_APPROVED=1 (owner approval); refusing"
            );
            return 1;
        }
        let passphrase = std::env::var("MAVEN_GPG_PASSPHRASE").unwrap_or_default();
        let workdir = match tempfile_stage_dir("maven-deploy-") {
            Ok(dir) => dir,
            Err(error) => {
                eprintln!("maven_deploy: {error}");
                return 1;
            }
        };
        let settings = match write_secure_file(
            &workdir,
            "settings.xml",
            &maven_settings_xml(&username, &password, &passphrase),
        ) {
            Ok(path) => path,
            Err(error) => {
                eprintln!("maven_deploy: {error}");
                let _ = std::fs::remove_dir_all(&workdir);
                return 1;
            }
        };
        let mut cmd = std::process::Command::new("mvn");
        cmd.arg("-B")
            .arg("--settings")
            .arg(&settings)
            .arg("deploy:deploy-file")
            .arg(format!("-Durl={repository_url}"))
            .arg("-DrepositoryId=dx-staging")
            .arg(format!("-Dfile={}", jar.display()))
            .arg(format!("-DpomFile={}", pom.display()))
            .arg(format!("-DgroupId={group}"))
            .arg(format!("-DartifactId={artifact}"))
            .arg(format!("-Dversion={version}"))
            .arg("-Dpackaging=jar");
        if !passphrase.is_empty() {
            cmd.arg("-Pgpg-sign");
        } else {
            if std::env::var("MAVEN_ALLOW_UNSIGNED").unwrap_or_default() != "1" {
                eprintln!(
                    "maven staging: no MAVEN_GPG_PASSPHRASE; refusing unsigned publish (set MAVEN_GPG_PASSPHRASE, or record unsigned staging explicitly with MAVEN_ALLOW_UNSIGNED=1 plus owner approval)"
                );
                let _ = std::fs::remove_dir_all(&workdir);
                return 1;
            }
            cmd.arg("-Dgpg.skip=true");
            println!(
                "maven_deploy: unsigned staging recorded (MAVEN_ALLOW_UNSIGNED=1 with owner approval; no GPG signature attached)"
            );
        }
        let status = cmd.status();
        let _ = std::fs::remove_dir_all(&workdir);
        match status {
            Ok(status) if status.success() => {
                println!("maven_deploy: staged {group}:{artifact}:{version}");
                return 0;
            }
            Ok(status) => {
                eprintln!("maven_deploy: mvn failed with {status}");
                return 1;
            }
            Err(error) => {
                eprintln!("maven_deploy: {error}");
                return 1;
            }
        }
    }
    match maven_build_file_repo(&jar, &pom, &outdir, group, artifact, version) {
        Ok(house) => {
            let profile = std::env::var("DX_PROFILE").unwrap_or_else(|_| "<unset>".to_owned());
            println!("maven_deploy: staged {group}:{artifact}:{version} (profile: {profile})");
            println!("  repo: {}", house.display());
            println!("  url: file://{}", house.display());
            0
        }
        Err(error) => {
            eprintln!("maven_deploy: {error}");
            1
        }
    }
}

/// Builds one OCI image layout.
pub fn oci_build_layout(
    image_tar_src: &Path,
    outdir: &Path,
    registry: &str,
    repository: &str,
    tag: &str,
) -> io::Result<std::path::PathBuf> {
    let src_str = image_tar_src.to_string_lossy();
    if !(src_str.ends_with(".tar") || src_str.ends_with(".tar.gz")) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("oci layout: want exactly one .tar source, got '{src_str}'"),
        ));
    }
    if registry.is_empty() || registry.contains("://") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("oci layout: want a registry host, got '{registry}'"),
        ));
    }
    if repository.is_empty() || repository.starts_with('/') || repository.ends_with('/') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("oci layout: want a repository path, got '{repository}'"),
        ));
    }
    if tag.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "oci layout: want a non-empty tag",
        ));
    }
    let base = repository.split('/').next_back().unwrap_or("");
    if base.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("oci layout: want a non-empty repository basename, got '{repository}'"),
        ));
    }
    let layout = outdir.join(base.to_owned() + "-oci-layout");
    let blobs = layout.join("blobs").join("sha256");
    std::fs::create_dir_all(&blobs)?;
    let layer_size = std::fs::metadata(image_tar_src)?.len();
    let layer_digest = format!("sha256:{}", sha256_file_hex(image_tar_src)?);
    let layer_blob = blobs.join(layer_digest.replace("sha256:", ""));
    std::fs::copy(image_tar_src, &layer_blob)?;
    if sha256_file_hex(&layer_blob)? != layer_digest.replace("sha256:", "") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "oci layout: byte mismatch for layer blob",
        ));
    }
    let layer_media = if src_str.ends_with(".tar.gz") {
        "application/vnd.oci.image.layer.v1.tar+gzip"
    } else {
        "application/vnd.oci.image.layer.v1.tar"
    };
    let config_body = format!(
        "{{\"architecture\":\"amd64\",\"created\":\"1970-01-01T00:00:00Z\",\"os\":\"linux\",\"rootfs\":{{\"diff_ids\":[\"{layer_digest}\"],\"type\":\"layers\"}}}}"
    );
    let config_bytes = format!("{config_body}\n");
    let config_digest = {
        use sha2::Digest as _;
        let mut hasher = sha2::Sha256::new();
        hasher.update(config_bytes.as_bytes());
        format!("sha256:{}", hex::encode(hasher.finalize()))
    };
    std::fs::write(
        blobs.join(config_digest.replace("sha256:", "")),
        config_bytes.as_bytes(),
    )?;
    let manifest_body = format!(
        "{{\"config\":{{\"digest\":\"{config_digest}\",\"mediaType\":\"application/vnd.oci.image.config.v1+json\",\"size\":{}}},\"layers\":[{{\"digest\":\"{layer_digest}\",\"mediaType\":\"{layer_media}\",\"size\":{layer_size}}}],\"mediaType\":\"application/vnd.oci.image.manifest.v1+json\",\"schemaVersion\":2}}",
        config_bytes.len(),
    );
    let manifest_bytes = format!("{manifest_body}\n");
    let manifest_digest = {
        use sha2::Digest as _;
        let mut hasher = sha2::Sha256::new();
        hasher.update(manifest_bytes.as_bytes());
        format!("sha256:{}", hex::encode(hasher.finalize()))
    };
    std::fs::write(
        blobs.join(manifest_digest.replace("sha256:", "")),
        manifest_bytes.as_bytes(),
    )?;
    let reference = format!("{registry}/{repository}:{tag}");
    let index_body = format!(
        "{{\"manifests\":[{{\"annotations\":{{\"org.opencontainers.image.ref.name\":\"{}\"}},\"digest\":\"{manifest_digest}\",\"mediaType\":\"application/vnd.oci.image.manifest.v1+json\",\"size\":{}}}],\"mediaType\":\"application/vnd.oci.image.index.v1+json\",\"schemaVersion\":2}}\n",
        json_escape(&reference),
        manifest_bytes.len(),
    );
    std::fs::write(layout.join("index.json"), index_body.as_bytes())?;
    std::fs::write(
        layout.join("oci-layout"),
        "{\"imageLayoutVersion\":\"1.0.0\"}\n".as_bytes(),
    )?;
    Ok(layout)
}

/// Runs the OCI deploy launcher.
pub fn oci_main(
    image_rloc: &str,
    registry: &str,
    repository: &str,
    tag: &str,
    argv: &[String],
) -> i32 {
    if argv.len() > 2 {
        eprintln!(
            "oci_deploy: this deploy target takes at most an output directory; the layout is exactly the file pinned at analysis time"
        );
        return 1;
    }
    let outdir = deploy_outdir(argv);
    if std::fs::create_dir_all(&outdir).is_err() {
        eprintln!("oci_deploy: cannot create {}", outdir.display());
        return 1;
    }
    let image = match resolve_runfile(image_rloc) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("oci_deploy: {error}");
            return 1;
        }
    };
    if std::env::var("OCI_PUBLISH_LIVE").unwrap_or_default() == "1" {
        let user = std::env::var("OCI_REGISTRY_USER").unwrap_or_default();
        let token = std::env::var("OCI_REGISTRY_TOKEN").unwrap_or_default();
        let approved = std::env::var("OCI_PUBLISH_APPROVED").unwrap_or_default();
        if tag == "0.0.0" || tag == "0.0.0-dryrun" {
            eprintln!("oci_deploy: live push refuses placeholder tag; set a real tag");
            return 1;
        }
        if user.is_empty() || token.is_empty() {
            eprintln!(
                "oci_deploy: live push needs OCI_REGISTRY_USER plus OCI_REGISTRY_TOKEN plus explicit owner approval (OCI_PUBLISH_APPROVED=1); refusing"
            );
            return 1;
        }
        if approved != "1" {
            eprintln!(
                "oci_deploy: live push needs OCI_PUBLISH_APPROVED=1 (owner approval); refusing"
            );
            return 1;
        }
        if which_on_path("docker").is_none() {
            eprintln!("oci_deploy: 'docker' CLI not found on PATH");
            return 1;
        }
        let reference = format!("{registry}/{repository}:{tag}");
        let mut login = std::process::Command::new("docker");
        login
            .arg("login")
            .arg(registry)
            .arg("-u")
            .arg(&user)
            .arg("--password-stdin");
        use std::io::Write as _;
        let mut child = match login.stdin(std::process::Stdio::piped()).spawn() {
            Ok(child) => child,
            Err(error) => {
                eprintln!("oci_deploy: {error}");
                return 1;
            }
        };
        if let Some(stdin) = child.stdin.take() {
            let mut stdin = stdin;
            let _ = stdin.write_all(token.as_bytes());
        }
        if !child.wait().map(|s| s.success()).unwrap_or(false) {
            eprintln!("oci_deploy: docker login failed");
            return 1;
        }
        let load = std::process::Command::new("docker")
            .arg("load")
            .arg("-i")
            .arg(&image)
            .status();
        if !load.map(|s| s.success()).unwrap_or(false) {
            eprintln!("oci_deploy: docker load failed");
            return 1;
        }
        match std::process::Command::new("docker")
            .arg("push")
            .arg(&reference)
            .status()
        {
            Ok(status) if status.success() => {
                println!("oci_deploy: pushed {reference}");
                println!(
                    "oci_deploy: signing-first: run cosign sign <digest> plus verify on the release trust root before updating scaffold refs"
                );
                return 0;
            }
            Ok(status) => {
                eprintln!("oci_deploy: docker push failed with {status}");
                return 1;
            }
            Err(error) => {
                eprintln!("oci_deploy: {error}");
                return 1;
            }
        }
    }
    if std::env::var("OCI_PUBLISH_DRY_RUN").unwrap_or_default() == "1" {
        println!(
            "oci_deploy: dry run (OCI_PUBLISH_DRY_RUN=1); would push {registry}/{repository}:{tag}"
        );
        println!(
            "oci_deploy: signing-first: verify SBOM/provenance plus signing_demo before any live push, then cosign sign <digest>"
        );
    }
    match oci_build_layout(&image, &outdir, registry, repository, tag) {
        Ok(layout) => {
            let profile = std::env::var("DX_PROFILE").unwrap_or_else(|_| "<unset>".to_owned());
            println!("oci_deploy: staged {registry}/{repository}:{tag} (profile: {profile})");
            println!("  layout: {}", layout.display());
            println!("  index: {}", layout.join("index.json").display());
            0
        }
        Err(error) => {
            eprintln!("oci_deploy: {error}");
            1
        }
    }
}

/// Formats one Octopus push command.
pub fn octopus_push_command(package_base: &str, server: &str, space: &str) -> String {
    let mut cmd = format!("octo push {package_base} --server {server}");
    if !space.is_empty() {
        cmd.push_str(&format!(" --space {space}"));
    }
    cmd
}

/// Formats one Octopus release command.
pub fn octopus_release_command(
    project: &str,
    channel: &str,
    version: &str,
    package_base: &str,
    deploy_to: &[String],
    space: &str,
    server: &str,
) -> String {
    let mut cmd = format!(
        "octo create-release --project {project} --channel {channel} --version {version} --package {package_base}"
    );
    for env in deploy_to {
        cmd.push_str(&format!(" --deploy-to {env}"));
    }
    if !space.is_empty() {
        cmd.push_str(&format!(" --space {space}"));
    }
    cmd.push_str(&format!(" --server {server}"));
    cmd
}

/// Octopus drop inputs.
pub struct OctopusDrop<'a> {
    pub deploy_name: &'a str,
    pub project: &'a str,
    pub channel: &'a str,
    pub version: &'a str,
    pub deploy_to: &'a [String],
    pub space: &'a str,
    pub server: &'a str,
}

/// Octopus launcher inputs.
pub struct OctopusLaunch<'a> {
    pub package_rloc: &'a str,
    pub deploy_name: &'a str,
    pub project: &'a str,
    pub channel: &'a str,
    pub version: &'a str,
    pub deploy_to_raw: &'a str,
    pub space_default: &'a str,
    pub url_default: &'a str,
}

/// Builds one Octopus package drop.
pub fn octopus_build_drop(
    package_src: &Path,
    outdir: &Path,
    spec: &OctopusDrop<'_>,
) -> io::Result<std::path::PathBuf> {
    let src_str = package_src.to_string_lossy();
    if !src_str.ends_with(".tar.gz") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "octopus drop: want exactly one .tar.gz source (archive_deploy output), got '{src_str}'"
            ),
        ));
    }
    if spec.deploy_name.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "octopus drop: need a non-empty deploy name",
        ));
    }
    if spec.project.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "octopus drop: need a non-empty project",
        ));
    }
    if spec.channel.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "octopus drop: need a non-empty channel",
        ));
    }
    if spec.version.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "octopus drop: need a non-empty version",
        ));
    }
    let drop = outdir.join(spec.deploy_name.to_owned() + "-drop");
    std::fs::create_dir_all(&drop)?;
    let base = basename(package_src)?;
    let dest = drop.join(&base);
    let digest = copy_verified(package_src, &dest)?;
    let manifest = format!(
        "# would-run manifest for octopus_deploy (local default publishes nothing)\n{}\n{}\npackage-sha256: {digest}  {base}\n",
        octopus_push_command(&base, spec.server, spec.space),
        octopus_release_command(spec.project, spec.channel, spec.version, &base, spec.deploy_to, spec.space, spec.server),
    );
    std::fs::write(drop.join("would-run.txt"), manifest.as_bytes())?;
    Ok(drop)
}

/// Runs the Octopus deploy launcher.
pub fn octopus_main(spec: &OctopusLaunch<'_>, argv: &[String]) -> i32 {
    if argv.len() > 2 {
        eprintln!(
            "octopus_deploy: this deploy target takes at most an output directory; the drop is exactly the file pinned at analysis time"
        );
        return 1;
    }
    let outdir = deploy_outdir(argv);
    if std::fs::create_dir_all(&outdir).is_err() {
        eprintln!("octopus_deploy: cannot create {}", outdir.display());
        return 1;
    }
    let package = match resolve_runfile(spec.package_rloc) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("octopus_deploy: {error}");
            return 1;
        }
    };
    let space = std::env::var("OCTOPUS_SPACE").unwrap_or_else(|_| spec.space_default.to_owned());
    let server = std::env::var("OCTOPUS_URL").unwrap_or_else(|_| spec.url_default.to_owned());
    let deploy_to: Vec<String> = spec
        .deploy_to_raw
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned())
        .collect();
    if std::env::var("OCTOPUS_PUBLISH_LIVE").unwrap_or_default() == "1" {
        let api_key = std::env::var("OCTOPUS_API_KEY").unwrap_or_default();
        let approved = std::env::var("OCTOPUS_PUBLISH_APPROVED").unwrap_or_default();
        if spec.version == "0.0.0" {
            eprintln!("octopus_deploy: live push refuses version 0.0.0; set a real version");
            return 1;
        }
        if server.is_empty() || server == "https://octopus.example.invalid" {
            eprintln!(
                "octopus_deploy: live push needs OCTOPUS_URL plus explicit owner approval (OCTOPUS_PUBLISH_APPROVED=1); refusing"
            );
            return 1;
        }
        if api_key.is_empty() {
            eprintln!(
                "octopus_deploy: live push needs OCTOPUS_API_KEY plus explicit owner approval (OCTOPUS_PUBLISH_APPROVED=1); refusing"
            );
            return 1;
        }
        if approved != "1" {
            eprintln!(
                "octopus_deploy: live push needs OCTOPUS_PUBLISH_APPROVED=1 (owner approval); refusing"
            );
            return 1;
        }
        let mut push = std::process::Command::new("octo");
        push.arg("push").arg(&package).arg("--server").arg(&server);
        if !space.is_empty() {
            push.arg("--space").arg(&space);
        }
        push.arg("--apiKey").arg(&api_key);
        match push.status() {
            Ok(status) if status.success() => {
                println!("octopus_deploy: pushed {} {}", spec.project, spec.version);
                return 0;
            }
            Ok(status) => {
                eprintln!("octopus_deploy: octo push failed with {status}");
                return 1;
            }
            Err(error) => {
                eprintln!("octopus_deploy: {error}");
                return 1;
            }
        }
    }
    match octopus_build_drop(
        &package,
        &outdir,
        &OctopusDrop {
            deploy_name: spec.deploy_name,
            project: spec.project,
            channel: spec.channel,
            version: spec.version,
            deploy_to: &deploy_to,
            space: &space,
            server: &server,
        },
    ) {
        Ok(drop) => {
            let profile = std::env::var("DX_PROFILE").unwrap_or_else(|_| "<unset>".to_owned());
            println!(
                "octopus_deploy: staged {} {} (profile: {profile})",
                spec.project, spec.version
            );
            println!("  drop: {}", drop.display());
            println!("  manifest: {}", drop.join("would-run.txt").display());
            0
        }
        Err(error) => {
            eprintln!("octopus_deploy: {error}");
            1
        }
    }
}

/// Formats one promotion command.
pub fn promotion_promote_command(
    artifact_base: &str,
    from_env: &str,
    to_env: &str,
    version: &str,
) -> String {
    format!("promote {artifact_base} {from_env} -> {to_env} version {version}")
}

/// Formats one rollback command.
pub fn promotion_rollback_command(deploy_name: &str, rollback_to: &str) -> String {
    format!(
        "rollback {deploy_name} to {rollback_to} (restore the pinned artifact for that version)"
    )
}

/// Builds one promotion staging dir.
pub fn promotion_build(
    artifact_src: &Path,
    outdir: &Path,
    deploy_name: &str,
    from_env: &str,
    to_env: &str,
    version: &str,
    secret_refs: &[String],
) -> io::Result<std::path::PathBuf> {
    if !artifact_src.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "promotion: want exactly one existing artifact source, got '{}'",
                artifact_src.display()
            ),
        ));
    }
    if deploy_name.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "promotion: need a non-empty deploy name",
        ));
    }
    if from_env.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "promotion: need a non-empty source environment",
        ));
    }
    if to_env.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "promotion: need a non-empty target environment",
        ));
    }
    if from_env == to_env {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("promotion: source and target environments must differ, got '{from_env}'"),
        ));
    }
    if version.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "promotion: need a non-empty version",
        ));
    }
    let promotion = outdir.join(deploy_name.to_owned() + "-promotion");
    std::fs::create_dir_all(&promotion)?;
    let base = basename(artifact_src)?;
    let dest = promotion.join(&base);
    let digest = copy_verified(artifact_src, &dest)?;
    let mut refs: Vec<String> = secret_refs.to_vec();
    refs.sort();
    let mut record = String::from("{\n");
    record.push_str(&format!("  \"artifact\": \"{}\",\n", json_escape(&base)));
    record.push_str(&format!("  \"artifact_sha256\": \"{digest}\",\n"));
    record.push_str(&format!(
        "  \"deploy\": \"{}\",\n",
        json_escape(deploy_name)
    ));
    record.push_str(&format!(
        "  \"from_environment\": \"{}\",\n",
        json_escape(from_env)
    ));
    record.push_str("  \"health\": \"skipped-local\",\n");
    record.push_str("  \"registry_auth\": \"env-only\",\n");
    record.push_str("  \"secret_refs\": [");
    for (index, r) in refs.iter().enumerate() {
        if index > 0 {
            record.push_str(", ");
        }
        record.push_str(&format!("\"{}\"", json_escape(r)));
    }
    record.push_str("],\n");
    record.push_str(&format!(
        "  \"to_environment\": \"{}\",\n",
        json_escape(to_env)
    ));
    record.push_str(&format!(
        "  \"version\": \"{}\"\n}}\n",
        json_escape(version)
    ));
    std::fs::write(promotion.join("promotion.json"), record.as_bytes())?;
    let refs_line = if refs.is_empty() {
        "secret-refs: none".to_owned()
    } else {
        format!("secret-refs: {} (names only, never values)", refs.join(","))
    };
    let manifest = format!(
        "# would-run manifest for promotion_deploy (local default publishes nothing; live needs PROMOTION_LIVE=1 plus PROMOTION_APPROVED=1)\n{}\nhealth: skipped locally; set PROMOTION_REQUIRE_HEALTH=1 plus PROMOTION_HEALTH_CMD to gate the promotion on a health check\nrollback: set PROMOTION_ROLLBACK=1 plus PROMOTION_ROLLBACK_TO=<version> to record a rollback pointer (history lives in the promotion dir or registry)\nartifact-sha256: {digest}  {base}\n{refs_line}\nregistry-auth: env-only (PROMOTION_REGISTRY_USER/PROMOTION_REGISTRY_TOKEN), never from BUILD\n",
        promotion_promote_command(&base, from_env, to_env, version),
    );
    std::fs::write(promotion.join("would-run.txt"), manifest.as_bytes())?;
    Ok(promotion)
}

/// Records one rollback pointer.
pub fn promotion_record_rollback(
    promotion_dir: &Path,
    deploy_name: &str,
    rollback_to: &str,
    rollback_sha: &str,
) -> io::Result<std::path::PathBuf> {
    if !promotion_dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "promotion rollback: want an existing promotion directory, got '{}'",
                promotion_dir.display()
            ),
        ));
    }
    if rollback_to.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "promotion rollback: need PROMOTION_ROLLBACK_TO=<version>",
        ));
    }
    let mut body = format!(
        "# rollback pointer for promotion_deploy (restores, never deletes)\n{}\n",
        promotion_rollback_command(deploy_name, rollback_to),
    );
    if !rollback_sha.is_empty() {
        body.push_str(&format!("expected-sha256: {rollback_sha}\n"));
    }
    let path = promotion_dir.join("rollback.txt");
    std::fs::write(&path, body.as_bytes())?;
    Ok(path)
}

/// Validates one promotion secret ref list.
pub fn promotion_check_secret_refs(secret_refs: &[String]) -> io::Result<Vec<String>> {
    let mut normalized = Vec::new();
    for r in secret_refs {
        if let Some(path) = r.strip_prefix("file:") {
            if path.is_empty() || !std::path::Path::new(path).exists() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("promotion secrets: secret file not found for ref '{r}'"),
                ));
            }
            normalized.push(r.clone());
        } else if let Some(rest) = r.strip_prefix("cmd:") {
            let tool = rest.split(' ').next().unwrap_or("");
            if tool.is_empty() || which_on_path(tool).is_none() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!(
                        "promotion secrets: secret tool not installed for ref '{r}' (install it separately and accept its license first)"
                    ),
                ));
            }
            normalized.push(r.clone());
        } else if r.is_empty() || r.chars().any(|c| " \n'\"\\$`".contains(c)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "promotion secrets: want ENV_NAME, file:<path>, or cmd:<tool> refs, got '{r}'"
                ),
            ));
        } else {
            if std::env::var(r).unwrap_or_default().is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("promotion secrets: env ref '{r}' is not set"),
                ));
            }
            normalized.push(r.clone());
        }
    }
    Ok(normalized)
}

/// Runs one promotion health command.
pub fn promotion_run_health_cmd(health_cmd: &str) -> io::Result<i32> {
    let argv = shlex_split(health_cmd);
    if argv.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "promotion health: need a non-empty PROMOTION_HEALTH_CMD",
        ));
    }
    if which_on_path(&argv[0]).is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("promotion health: command not found on PATH: '{}'", argv[0]),
        ));
    }
    let mut cmd = std::process::Command::new(&argv[0]);
    for arg in &argv[1..] {
        cmd.arg(arg);
    }
    Ok(cmd.status()?.code().unwrap_or(1))
}

fn shlex_split(cmd: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = cmd.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            } else if c == '\\' && q == '"' {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            } else {
                current.push(c);
            }
        } else if c == '\'' || c == '"' {
            quote = Some(c);
        } else if c == '\\' {
            if let Some(next) = chars.next() {
                current.push(next);
            }
        } else if c.is_whitespace() {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Runs the promotion deploy launcher.
pub fn promotion_main(
    artifact_rloc: &str,
    deploy_name: &str,
    from_env: &str,
    to_env: &str,
    version: &str,
    argv: &[String],
) -> i32 {
    if argv.len() > 2 {
        eprintln!(
            "promotion_deploy: this deploy target takes at most an output directory; the promotion is exactly the file pinned at analysis time"
        );
        return 1;
    }
    let outdir = deploy_outdir(argv);
    if std::fs::create_dir_all(&outdir).is_err() {
        eprintln!("promotion_deploy: cannot create {}", outdir.display());
        return 1;
    }
    let artifact = match resolve_runfile(artifact_rloc) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("promotion_deploy: {error}");
            return 1;
        }
    };
    let raw_refs = std::env::var("PROMOTION_SECRET_REFS").unwrap_or_default();
    let secret_refs: Vec<String> = raw_refs
        .split(',')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    if std::env::var("PROMOTION_ROLLBACK").unwrap_or_default() == "1" {
        let promotion = match promotion_build(
            &artifact,
            &outdir,
            deploy_name,
            from_env,
            to_env,
            version,
            &secret_refs,
        ) {
            Ok(promotion) => promotion,
            Err(error) => {
                eprintln!("promotion_deploy: {error}");
                return 1;
            }
        };
        let rollback_to = std::env::var("PROMOTION_ROLLBACK_TO").unwrap_or_default();
        let rollback_sha = std::env::var("PROMOTION_ROLLBACK_SHA").unwrap_or_default();
        if let Err(error) =
            promotion_record_rollback(&promotion, deploy_name, &rollback_to, &rollback_sha)
        {
            eprintln!("promotion_deploy: {error}");
            return 1;
        }
        println!("promotion_deploy: recorded rollback for {deploy_name} to {rollback_to}");
        println!("  promotion: {}", promotion.display());
        return 0;
    }
    if std::env::var("PROMOTION_LIVE").unwrap_or_default() == "1" {
        let approved = std::env::var("PROMOTION_APPROVED").unwrap_or_default();
        if version == "0.0.0" {
            eprintln!("promotion_deploy: live promote refuses version 0.0.0; set a real version");
            return 1;
        }
        if approved != "1" {
            eprintln!(
                "promotion_deploy: live promote needs PROMOTION_APPROVED=1 (owner approval); refusing"
            );
            return 1;
        }
        if std::env::var("PROMOTION_REGISTRY_REQUIRED").unwrap_or_default() == "1"
            && (std::env::var("PROMOTION_REGISTRY_USER")
                .unwrap_or_default()
                .is_empty()
                || std::env::var("PROMOTION_REGISTRY_TOKEN")
                    .unwrap_or_default()
                    .is_empty())
        {
            eprintln!(
                "promotion_deploy: live promote needs PROMOTION_REGISTRY_USER plus PROMOTION_REGISTRY_TOKEN (env only, never BUILD); refusing"
            );
            return 1;
        }
        if let Err(error) = promotion_check_secret_refs(&secret_refs) {
            eprintln!("promotion_deploy: {error}");
            return 1;
        }
        if std::env::var("PROMOTION_REQUIRE_HEALTH").unwrap_or_default() == "1" {
            let health_cmd = std::env::var("PROMOTION_HEALTH_CMD").unwrap_or_default();
            if health_cmd.is_empty() {
                eprintln!(
                    "promotion_deploy: PROMOTION_REQUIRE_HEALTH=1 needs PROMOTION_HEALTH_CMD; refusing"
                );
                return 1;
            }
            match promotion_run_health_cmd(&health_cmd) {
                Ok(0) => {}
                Ok(code) => {
                    eprintln!(
                        "promotion_deploy: health check failed (exit {code}); refusing to promote"
                    );
                    return 1;
                }
                Err(error) => {
                    eprintln!("promotion_deploy: {error}");
                    return 1;
                }
            }
        }
        match promotion_build(
            &artifact,
            &outdir,
            deploy_name,
            from_env,
            to_env,
            version,
            &secret_refs,
        ) {
            Ok(promotion) => {
                println!(
                    "promotion_deploy: promoted {from_env} -> {to_env} version {version} (secrets by reference only, registry auth env-only)"
                );
                println!("  promotion: {}", promotion.display());
                return 0;
            }
            Err(error) => {
                eprintln!("promotion_deploy: {error}");
                return 1;
            }
        }
    }
    if std::env::var("PROMOTION_REQUIRE_HEALTH").unwrap_or_default() == "1"
        || std::env::var("PROMOTION_REGISTRY_REQUIRED").unwrap_or_default() == "1"
    {
        eprintln!(
            "promotion_deploy: local default stages only; health and registry gates apply to the live path (PROMOTION_LIVE=1); staged without running them"
        );
    }
    match promotion_build(
        &artifact,
        &outdir,
        deploy_name,
        from_env,
        to_env,
        version,
        &secret_refs,
    ) {
        Ok(promotion) => {
            let profile = std::env::var("DX_PROFILE").unwrap_or_else(|_| "<unset>".to_owned());
            println!(
                "promotion_deploy: staged {from_env} -> {to_env} version {version} (profile: {profile})"
            );
            println!("  promotion: {}", promotion.display());
            println!("  manifest: {}", promotion.join("would-run.txt").display());
            0
        }
        Err(error) => {
            eprintln!("promotion_deploy: {error}");
            1
        }
    }
}

fn tar_header_path(name: &str, size: u64, mode: u32) -> io::Result<[u8; BLOCK]> {
    let name_bytes = name.as_bytes();
    if name_bytes.is_empty() || name_bytes.len() > 100 || name_bytes.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("tar member name {name:?} must be 1-100 bytes with no NUL"),
        ));
    }
    let mut header = [0u8; BLOCK];
    header[0..name_bytes.len()].copy_from_slice(name_bytes);
    let mode_field = octal_field(u64::from(mode & 0o7777), 8)?;
    header[100..108].copy_from_slice(&mode_field);
    let uid_field = octal_field(UID, 8)?;
    header[108..116].copy_from_slice(&uid_field);
    let gid_field = octal_field(GID, 8)?;
    header[116..124].copy_from_slice(&gid_field);
    let size_field = octal_field(size, 12)?;
    header[124..136].copy_from_slice(&size_field);
    let mtime_field = octal_field(MTIME, 12)?;
    header[136..148].copy_from_slice(&mtime_field);
    for slot in header.iter_mut().take(156).skip(148) {
        *slot = b' ';
    }
    header[156] = b'0';
    header[257..265].copy_from_slice(b"ustar\x0000");
    let mut sum: u32 = 0;
    for byte in header {
        sum += u32::from(byte);
    }
    let checksum_text = format!("{sum:06o}\0 ");
    header[148..156].copy_from_slice(checksum_text.as_bytes());
    Ok(header)
}

/// Creates one deterministic npm pack.
pub fn npm_create_pack(
    package: &str,
    tag: &str,
    registry: &str,
    tgz_out: &Path,
    feed_out: &Path,
    srcs: &[std::path::PathBuf],
) -> io::Result<String> {
    let mut members: std::collections::BTreeMap<String, (Vec<u8>, u32)> =
        std::collections::BTreeMap::new();
    for src in srcs {
        let data = std::fs::read(src)?;
        let base = basename(src)?;
        let member = format!("package/{base}");
        if members.contains_key(&member) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("duplicate pack member '{member}'"),
            ));
        }
        let mode = {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                let bits = std::fs::metadata(src)?.permissions().mode();
                if bits & 0o111 != 0 {
                    0o755
                } else {
                    0o644
                }
            }
            #[cfg(not(unix))]
            {
                0o644
            }
        };
        members.insert(member, (data, mode));
    }
    let mut tar: Vec<u8> = Vec::new();
    for (member, (data, mode)) in &members {
        let header = tar_header_path(member, data.len() as u64, *mode)?;
        tar.extend_from_slice(&header);
        tar.extend_from_slice(data);
        let pad = (BLOCK - data.len() % BLOCK) % BLOCK;
        tar.extend(std::iter::repeat_n(0, pad));
    }
    tar.extend(std::iter::repeat_n(0, 2 * BLOCK));
    while !tar.len().is_multiple_of(RECORDSIZE) {
        tar.extend(std::iter::repeat_n(0, BLOCK));
    }
    let gz = gzip_compress(&tar)?;
    if let Some(parent) = tgz_out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(tgz_out, &gz)?;
    let digest = sha256_file_hex(tgz_out)?;
    let names: Vec<String> = members.keys().cloned().collect();
    let tgz_base = basename(tgz_out)?;
    let mut feed = String::from("{\n");
    feed.push_str("  \"files\": [");
    for (index, name) in names.iter().enumerate() {
        if index > 0 {
            feed.push_str(", ");
        }
        feed.push_str(&format!("\"{}\"", json_escape(name)));
    }
    feed.push_str("],\n");
    feed.push_str(&format!("  \"name\": \"{}\",\n", json_escape(package)));
    feed.push_str(&format!("  \"registry\": \"{}\",\n", json_escape(registry)));
    feed.push_str(&format!("  \"sha256\": \"{digest}\",\n"));
    feed.push_str(&format!("  \"tag\": \"{}\",\n", json_escape(tag)));
    feed.push_str(&format!(
        "  \"tarball\": \"{}\"\n}}\n",
        json_escape(&tgz_base)
    ));
    if let Some(parent) = feed_out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(feed_out, feed.as_bytes())?;
    Ok(digest)
}

/// Lists one gzip tar member table.
pub fn npm_tar_members(tgz_path: &Path) -> io::Result<Vec<String>> {
    let gz = std::fs::read(tgz_path)?;
    let mut decoder = flate2::read::GzDecoder::new(&gz[..]);
    let mut tar = Vec::new();
    use std::io::Read as _;
    decoder.read_to_end(&mut tar)?;
    let mut members = Vec::new();
    let mut offset = 0;
    while offset + BLOCK <= tar.len() {
        let header = &tar[offset..offset + BLOCK];
        if header.iter().all(|b| *b == 0) {
            break;
        }
        let end = header.iter().position(|b| *b == 0).unwrap_or(100);
        let name = String::from_utf8_lossy(&header[0..end]).into_owned();
        let size_text = String::from_utf8_lossy(&header[124..136]).into_owned();
        let size_text = size_text.trim_matches(|c| c == '\0' || c == ' ');
        let size = u64::from_str_radix(size_text.trim(), 8).unwrap_or(0);
        members.push(name);
        offset += BLOCK;
        let blocks = (size as usize).div_ceil(BLOCK);
        offset += blocks * BLOCK;
    }
    Ok(members)
}

/// Renders one npmrc auth line.
pub fn npm_npmrc_line(host: &str, token: &str) -> String {
    format!("//{host}/:_authToken={token}\n")
}

/// Writes one owner-only npmrc.
pub fn npm_write_npmrc(host: &str, token: &str) -> io::Result<std::path::PathBuf> {
    let mut path = std::env::temp_dir();
    path.push(format!(
        ".npmrc-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::write(&path, npm_npmrc_line(host, token).as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(path)
}

fn json_string_field(body: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let start = body.find(&needle)?;
    let rest = &body[start + needle.len()..];
    let colon = rest.find(':')?;
    let mut value = rest[colon + 1..].trim_start();
    if !value.starts_with('"') {
        return None;
    }
    value = &value[1..];
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '"' {
            return Some(out);
        }
        if c == '\\' {
            if let Some(next) = chars.next() {
                match next {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    _ => {
                        out.push('\\');
                        out.push(next);
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    None
}

fn json_files_field(body: &str) -> Option<Vec<String>> {
    let needle = "\"files\"";
    let start = body.find(needle)?;
    let rest = &body[start + needle.len()..];
    let open = rest.find('[')?;
    let after = &rest[open + 1..];
    let close = after.find(']')?;
    let inner = &after[..close];
    let mut out = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    let mut current = String::new();
    for c in inner.chars() {
        if in_string {
            if escaped {
                match c {
                    '"' => current.push('"'),
                    '\\' => current.push('\\'),
                    'n' => current.push('\n'),
                    _ => {
                        current.push('\\');
                        current.push(c);
                    }
                }
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
                out.push(std::mem::take(&mut current));
            } else {
                current.push(c);
            }
        } else if c == '"' {
            in_string = true;
        }
    }
    Some(out)
}

/// Verifies one npm pack pair.
pub fn npm_verify_pack(tgz_path: &Path, feed_path: &Path) -> io::Result<(String, String, String)> {
    let body = std::fs::read_to_string(feed_path)?;
    let name = json_string_field(&body, "name")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "feed JSON is missing 'name'"))?;
    let tag = json_string_field(&body, "tag")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "feed JSON is missing 'tag'"))?;
    let registry = json_string_field(&body, "registry").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "feed JSON is missing 'registry'",
        )
    })?;
    let _tarball = json_string_field(&body, "tarball").ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "feed JSON is missing 'tarball'")
    })?;
    let expected = json_string_field(&body, "sha256").ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "feed JSON is missing 'sha256'")
    })?;
    let files = json_files_field(&body).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "feed JSON 'files' must be a list",
        )
    })?;
    if !body.contains("\"files\"") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "feed JSON is missing 'files'",
        ));
    }
    let actual = sha256_file_hex(tgz_path)?;
    if expected != actual {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "checksum mismatch for {} (expected {expected}, actual {actual})",
                tgz_path.display(),
            ),
        ));
    }
    let members = npm_tar_members(tgz_path)?;
    if members != files {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "pack members mismatch for {}: {members:?}",
                tgz_path.display()
            ),
        ));
    }
    Ok((name, tag, registry))
}

fn runfiles_manifest_entries() -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Ok(manifest) = std::env::var("RUNFILES_MANIFEST_FILE") {
        if let Ok(text) = std::fs::read_to_string(&manifest) {
            for line in text.lines() {
                let mut parts = line.splitn(2, ' ');
                if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                    out.push((key.to_owned(), value.to_owned()));
                }
            }
        }
    }
    out
}

fn runfiles_dir_walk(root: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            runfiles_dir_walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Finds one npm pack pair in runfiles.
pub fn npm_find_pack_inputs() -> io::Result<(std::path::PathBuf, std::path::PathBuf)> {
    let manifest = runfiles_manifest_entries();
    let mut feeds: Vec<std::path::PathBuf> = Vec::new();
    if !manifest.is_empty() {
        for (rloc, real) in &manifest {
            if rloc.ends_with(".feed.json") {
                feeds.push(std::path::PathBuf::from(real));
            }
        }
    }
    if feeds.is_empty() {
        if let Ok(dir) = std::env::var("RUNFILES_DIR") {
            let root = std::path::PathBuf::from(dir);
            let mut all = Vec::new();
            runfiles_dir_walk(&root, &mut all);
            for path in all {
                if path.extension().and_then(|e| e.to_str()) == Some("json")
                    && path.to_string_lossy().ends_with(".feed.json")
                {
                    feeds.push(path);
                }
            }
        }
    }
    if feeds.len() != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "want exactly one .feed.json in runfiles, found {}",
                feeds.len()
            ),
        ));
    }
    let feed_path = feeds.remove(0);
    let body = std::fs::read_to_string(&feed_path)?;
    let tarball = json_string_field(&body, "tarball").ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "feed JSON is missing 'tarball'")
    })?;
    let mut tgzs = Vec::new();
    if !manifest.is_empty() {
        for (rloc, real) in &manifest {
            if rloc == &tarball || rloc.ends_with(&format!("/{tarball}")) {
                tgzs.push(std::path::PathBuf::from(real));
            }
        }
    }
    if tgzs.is_empty() {
        if let Ok(dir) = std::env::var("RUNFILES_DIR") {
            let root = std::path::PathBuf::from(dir);
            let mut all = Vec::new();
            runfiles_dir_walk(&root, &mut all);
            for path in all {
                let rel = path.to_string_lossy().into_owned();
                if rel == tarball || rel.ends_with(&format!("/{tarball}")) {
                    tgzs.push(path);
                } else if let Ok(base) = basename(&path) {
                    if base == tarball {
                        tgzs.push(path);
                    }
                }
            }
        }
    }
    if tgzs.len() != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "want exactly one '{tarball}' in runfiles, found {}",
                tgzs.len()
            ),
        ));
    }
    Ok((feed_path, tgzs.remove(0)))
}

/// Runs the npm pack tool.
pub fn npm_pack_main(argv: &[String]) -> i32 {
    if argv.len() < 7 {
        eprintln!("usage: npm_pack <package> <tag> <registry> <tgz_out> <feed_out> <src...>");
        return 1;
    }
    let package = argv[1].clone();
    let tag = argv[2].clone();
    let registry = argv[3].clone();
    let tgz_out = std::path::PathBuf::from(&argv[4]);
    let feed_out = std::path::PathBuf::from(&argv[5]);
    let srcs: Vec<std::path::PathBuf> = argv[6..].iter().map(std::path::PathBuf::from).collect();
    match npm_create_pack(&package, &tag, &registry, &tgz_out, &feed_out, &srcs) {
        Ok(_) => 0,
        Err(error) => {
            eprintln!("npm_pack: {error}");
            1
        }
    }
}

/// Runs the npm deploy launcher.
pub fn npm_deploy_main(argv: &[String]) -> i32 {
    let outdir = deploy_outdir(argv);
    if std::fs::create_dir_all(&outdir).is_err() {
        eprintln!("npm_deploy: cannot create {}", outdir.display());
        return 1;
    }
    let (feed_path, tgz_path) = match npm_find_pack_inputs() {
        Ok(pair) => pair,
        Err(error) => {
            eprintln!("npm_deploy: {error}");
            return 1;
        }
    };
    let (name, tag, registry) = match npm_verify_pack(&tgz_path, &feed_path) {
        Ok(triple) => triple,
        Err(error) => {
            eprintln!("npm_deploy: {error}");
            return 1;
        }
    };
    let tgz_base = match basename(&tgz_path) {
        Ok(base) => base,
        Err(error) => {
            eprintln!("npm_deploy: {error}");
            return 1;
        }
    };
    let feed_base = match basename(&feed_path) {
        Ok(base) => base,
        Err(error) => {
            eprintln!("npm_deploy: {error}");
            return 1;
        }
    };
    let stem = tgz_base
        .strip_suffix(".tgz")
        .unwrap_or(&tgz_base)
        .to_owned();
    let feed_dir = outdir.join(stem.clone() + "-feed");
    if std::fs::create_dir_all(&feed_dir).is_err() {
        eprintln!("npm_deploy: cannot create {}", feed_dir.display());
        return 1;
    }
    for (src, base) in [(&tgz_path, &tgz_base), (&feed_path, &feed_base)] {
        if std::fs::copy(src, outdir.join(base)).is_err() {
            eprintln!("npm_deploy: cannot copy {}", src.display());
            return 1;
        }
        if std::fs::copy(src, feed_dir.join(base)).is_err() {
            eprintln!("npm_deploy: cannot copy {}", src.display());
            return 1;
        }
    }
    let profile = std::env::var("DX_PROFILE").unwrap_or_else(|_| "<unset>".to_owned());
    println!("npm_deploy: released {name} (tag {tag}, profile: {profile})");
    println!("  tarball: {}", outdir.join(&tgz_base).display());
    println!("  feed: {}", feed_dir.display());
    if std::env::var("NPM_PUBLISH_LIVE").unwrap_or_default() == "1" {
        let token = std::env::var("NPM_TOKEN").unwrap_or_default();
        if token.is_empty() {
            eprintln!(
                "npm_deploy: live publish needs NPM_TOKEN plus explicit owner approval; publishing nothing"
            );
            return 1;
        }
        let staged_tgz = outdir.join(&tgz_base);
        let mut cmd = std::process::Command::new("npm");
        cmd.arg("publish")
            .arg(&staged_tgz)
            .arg("--tag")
            .arg(&tag)
            .arg("--access")
            .arg("public")
            .arg("--provenance")
            .arg("--registry")
            .arg(&registry);
        if std::env::var("NPM_PUBLISH_DRY_RUN").unwrap_or_default() == "1" {
            println!("npm_deploy: dry run (NPM_PUBLISH_DRY_RUN=1); would publish:");
            println!("  package: {name}");
            println!("  tag: {tag}");
            println!("  command: npm publish {} --tag {tag} --access public --provenance --registry {registry}", staged_tgz.display());
            return 0;
        }
        if which_on_path("npm").is_none() {
            eprintln!("npm_deploy: 'npm' CLI not found on PATH");
            return 1;
        }
        let host = registry
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or("");
        if host.is_empty() {
            eprintln!("npm_deploy: invalid registry '{registry}'");
            return 1;
        }
        let npmrc = match npm_write_npmrc(host, &token) {
            Ok(path) => path,
            Err(error) => {
                eprintln!("npm_deploy: {error}");
                return 1;
            }
        };
        let status = std::process::Command::new("npm")
            .arg("publish")
            .arg(&staged_tgz)
            .arg("--tag")
            .arg(&tag)
            .arg("--access")
            .arg("public")
            .arg("--provenance")
            .arg("--registry")
            .arg(&registry)
            .arg("--userconfig")
            .arg(&npmrc)
            .status();
        let _ = std::fs::remove_file(&npmrc);
        match status {
            Ok(status) => {
                return if status.success() {
                    0
                } else {
                    status.code().unwrap_or(1)
                }
            }
            Err(error) => {
                eprintln!("npm_deploy: {error}");
                return 1;
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn scratch_dir() -> tempfile::TempDir {
        tempfile::TempDir::new().expect("scratch")
    }

    fn with_env<T>(vars: &[(&str, Option<&str>)], body: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let mut saved: Vec<(&str, Option<String>)> = Vec::new();
        for (key, value) in vars {
            saved.push((*key, std::env::var(key).ok()));
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
        let out = body();
        for (key, value) in saved {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
        out
    }

    #[test]
    fn octal_fields_match_python_tarfile() {
        assert_eq!(octal_field(0o755, 8).expect("mode"), b"0000755\0");
        assert_eq!(octal_field(0, 8).expect("uid"), b"0000000\0");
        assert_eq!(octal_field(12, 12).expect("size"), b"00000000014\0");
        assert_eq!(octal_field(0, 12).expect("mtime"), b"00000000000\0");
        assert!(octal_field(8u64.pow(7), 8).is_err());
        assert!(octal_field(8u64.pow(11), 12).is_err());
    }

    #[test]
    fn header_bytes_match_python_ustar() {
        let header = tar_header("hello", 12, 0o755).expect("header");
        assert_eq!(&header[0..6], b"hello\0");
        assert!(header[6..100].iter().all(|byte| *byte == 0));
        assert_eq!(&header[100..108], b"0000755\0");
        assert_eq!(&header[108..116], b"0000000\0");
        assert_eq!(&header[116..124], b"0000000\0");
        assert_eq!(&header[124..136], b"00000000014\0");
        assert_eq!(&header[136..148], b"00000000000\0");
        assert_eq!(&header[148..156], b"006771\0 ");
        assert_eq!(header[156], b'0');
        assert!(header[157..257].iter().all(|byte| *byte == 0));
        assert_eq!(&header[257..265], b"ustar\x0000");
        assert!(header[265..512].iter().all(|byte| *byte == 0));
        let plain = tar_header("hello", 12, 0o644).expect("plain header");
        assert_eq!(&plain[100..108], b"0000644\0");
        assert_ne!(&plain[148..156], &header[148..156]);
    }

    #[test]
    fn header_rejects_bad_names() {
        assert!(tar_header("", 0, 0o644).is_err());
        assert!(tar_header(&"n".repeat(101), 0, 0o644).is_err());
        assert!(tar_header("a/b", 0, 0o644).is_err());
        assert!(tar_header("a\0b", 0, 0o644).is_err());
    }

    #[test]
    fn tar_image_pads_to_recordsize() {
        let tiny = tar_image("f", b"hi", 0o644).expect("tiny");
        assert_eq!(tiny.len(), RECORDSIZE);
        assert_eq!(&tiny[0..1], b"f");
        assert_eq!(&tiny[512..514], b"hi");
        assert!(tiny[514..].iter().all(|byte| *byte == 0));
        let big_data = vec![b'x'; 10240];
        let big = tar_image("f", &big_data, 0o644).expect("big");
        assert_eq!(big.len(), 2 * RECORDSIZE);
    }

    #[test]
    fn gzip_header_matches_python_gzip() {
        let tar = tar_image("hello", b"hello world\n", 0o755).expect("tar");
        let gz = gzip_compress(&tar).expect("gz");
        assert_eq!(
            &gz[0..10],
            &[0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0xff]
        );
        let mut decoder = flate2::read::GzDecoder::new(&gz[..]);
        let mut decoded = Vec::new();
        use std::io::Read as _;
        decoder.read_to_end(&mut decoded).expect("decode");
        assert_eq!(decoded, tar);
    }

    #[test]
    fn archive_bytes_match_python_golden() {
        let gz = archive_bytes(b"hello world\n", "hello", true).expect("archive");
        assert_eq!(gz.len(), 110);
        assert_eq!(
            &gz[0..10],
            &[0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0xff]
        );
        let mut decoder = flate2::read::GzDecoder::new(&gz[..]);
        let mut tar = Vec::new();
        use std::io::Read as _;
        decoder.read_to_end(&mut tar).expect("decode");
        assert_eq!(tar.len(), RECORDSIZE);
        assert_eq!(&tar[0..6], b"hello\0");
        assert_eq!(&tar[100..108], b"0000755\0");
        assert_eq!(&tar[148..156], b"006771\0 ");
        assert_eq!(&tar[512..524], b"hello world\n");
    }

    #[test]
    fn archive_file_round_trips_modes_and_symlinks() {
        let scratch = scratch_dir();
        let executable = scratch.path().join("run.sh");
        std::fs::write(&executable, b"#!/bin/sh\necho hi\n").expect("write exe");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755))
                .expect("chmod");
        }
        let plain = scratch.path().join("data.txt");
        std::fs::write(&plain, b"plain\n").expect("write plain");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644))
                .expect("chmod");
        }
        let exe_out = scratch.path().join("exe.tar.gz");
        archive_file(&executable, &exe_out).expect("archive exe");
        let plain_out = scratch.path().join("plain.tar.gz");
        archive_file(&plain, &plain_out).expect("archive plain");
        let exe_mode = if cfg!(unix) { "0000755" } else { "0000644" };
        for (path, want_mode) in [(&exe_out, exe_mode), (&plain_out, "0000644")] {
            let gz = std::fs::read(path).expect("read gz");
            let mut decoder = flate2::read::GzDecoder::new(&gz[..]);
            let mut tar = Vec::new();
            use std::io::Read as _;
            decoder.read_to_end(&mut tar).expect("decode");
            let mode = String::from_utf8_lossy(&tar[100..107]).into_owned();
            assert_eq!(mode, want_mode);
        }
        #[cfg(unix)]
        {
            let link = scratch.path().join("link.sh");
            std::os::unix::fs::symlink(&executable, &link).expect("symlink");
            let link_out = scratch.path().join("link.tar.gz");
            archive_file(&link, &link_out).expect("archive link");
            let gz = std::fs::read(&link_out).expect("read link gz");
            let mut decoder = flate2::read::GzDecoder::new(&gz[..]);
            let mut tar = Vec::new();
            use std::io::Read as _;
            decoder.read_to_end(&mut tar).expect("decode link");
            assert_eq!(&tar[0..7], b"link.sh");
            assert_eq!(&tar[512..530], b"#!/bin/sh\necho hi\n");
        }
    }

    #[test]
    fn archive_file_rejects_missing_basename() {
        let scratch = scratch_dir();
        let missing = scratch.path().join("does-not-exist");
        let out = scratch.path().join("out.tar.gz");
        assert!(archive_file(&missing, &out).is_err());
        assert!(basename(Path::new("/")).is_err());
    }

    #[test]
    fn sha256_vectors_match_hashlib() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hash_line(b"hello world\n", "release.tar.gz"),
            format!("{}  release.tar.gz\n", sha256_hex(b"hello world\n"))
        );
    }

    #[test]
    fn hash_file_writes_sha256sum_line() {
        let scratch = scratch_dir();
        let src = scratch.path().join("payload.bin");
        let payload = b"archive payload\n";
        std::fs::write(&src, payload).expect("write");
        let dst = scratch.path().join("payload.sha256");
        hash_file(&src, &dst).expect("hash");
        let text = std::fs::read_to_string(&dst).expect("read line");
        assert_eq!(text, hash_line(payload, "payload.bin"));
        assert!(text.ends_with('\n'));
        assert!(!text.ends_with("\r\n"));
        let parts: Vec<&str> = text.trim_end().split("  ").collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[1], "payload.bin");
        assert_eq!(parts[0].len(), 64);
    }

    #[test]
    fn hash_file_rejects_missing_inputs() {
        let scratch = scratch_dir();
        let missing = scratch.path().join("missing");
        let out = scratch.path().join("out.sha256");
        assert!(hash_file(&missing, &out).is_err());
    }

    #[test]
    fn bin_shims_share_usage_and_failure_exit() {
        assert_eq!(bin_usage("dx_archiver", "<src> <dst>"), 1);
        assert_eq!(
            bin_cannot(
                "dx_archiver",
                "write",
                Path::new("/tmp/out.tar.gz"),
                "read-only filesystem"
            ),
            1
        );
    }

    #[test]
    fn archive_stage_verifies_and_copies() {
        let scratch = scratch_dir();
        let app = scratch.path().join("deploy_program");
        std::fs::write(&app, b"deploy-program-bytes-v1").expect("write app");
        let tarball = scratch.path().join("release_demo.tar.gz");
        std::fs::write(&tarball, b"archive-tarball-bytes-v1").expect("write tarball");
        let digest = sha256_file_hex(&tarball).expect("digest");
        let checksum = scratch.path().join("release_demo.tar.gz.sha256");
        std::fs::write(
            &checksum,
            format!("{digest}  release_demo.tar.gz\n").as_bytes(),
        )
        .expect("write checksum");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let (app_name, tarball_dest, checksum_dest) =
            archive_stage_release(&app, &tarball, &checksum, &outdir).expect("stage");
        assert_eq!(app_name, "deploy_program");
        assert!(tarball_dest.is_file());
        assert!(checksum_dest.is_file());
        assert_eq!(
            sha256_file_hex(&tarball_dest).expect("hex"),
            sha256_file_hex(&tarball).expect("hex")
        );
        let bad_checksum = scratch.path().join("bad.sha256");
        std::fs::write(
            &bad_checksum,
            format!("{}  release_demo.tar.gz\n", "0".repeat(64)).as_bytes(),
        )
        .expect("write bad");
        assert!(archive_stage_release(&app, &tarball, &bad_checksum, &outdir).is_err());
    }

    #[test]
    fn pypi_wheelhouse_builds_index() {
        let scratch = scratch_dir();
        let wheel = scratch.path().join("pypi_demo-0.0.0-py3-none-any.whl");
        std::fs::write(&wheel, b"pypi-demo-wheel-bytes-v1").expect("write wheel");
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
                sha256_file_hex(&dest).expect("hex"),
                sha256_file_hex(src).expect("hex")
            );
        }
        let index = house
            .join("simple-index")
            .join("pypi_demo")
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
        assert!(house
            .join("vendor")
            .join("crates_demo")
            .join(".cargo-checksum.json")
            .is_file());
        assert!(house
            .join("registry")
            .join("crates_demo")
            .join("0.0.0")
            .join("index.json")
            .is_file());
        assert!(crates_build_vendor(&[], &outdir, "crates_demo", "0.0.0").is_err());
    }

    #[test]
    fn github_staging_writes_manifest() {
        let scratch = scratch_dir();
        let asset = scratch.path().join("release_demo.tar.gz");
        std::fs::write(&asset, b"asset-bytes-v1").expect("write asset");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let staging = github_build_staging(
            std::slice::from_ref(&asset),
            &outdir,
            "github_demo",
            "v0.0.0-dryrun",
        )
        .expect("staging");
        assert!(staging.join("release_demo.tar.gz").is_file());
        assert!(staging.join("would-run.txt").is_file());
        assert!(github_release_command("v1", &["a".to_owned()]).contains("gh release create v1 a"));
    }

    #[test]
    fn maven_repo_builds_layout_and_settings() {
        let scratch = scratch_dir();
        let jar = scratch.path().join("maven_demo-0.0.0.jar");
        std::fs::write(&jar, b"maven-demo-jar-bytes-v1").expect("write jar");
        let pom = scratch.path().join("maven_demo-0.0.0.pom");
        std::fs::write(&pom, b"<project>maven-demo-pom-v1</project>").expect("write pom");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let house =
            maven_build_file_repo(&jar, &pom, &outdir, "com.example", "maven_demo", "0.0.0")
                .expect("repo");
        assert!(house
            .join("com/example/maven_demo/0.0.0/maven_demo-0.0.0.jar")
            .is_file());
        assert!(house
            .join("com/example/maven_demo/maven-metadata.xml")
            .is_file());
        let settings = maven_settings_xml("user", "pass", "secret");
        assert!(settings.contains("dx-staging"));
        assert!(settings.contains("gpg.passphrase"));
        let plain = maven_settings_xml("user", "pass", "");
        assert!(!plain.contains("gpg.passphrase"));
        let secure = write_secure_file(scratch.path(), "settings.xml", &settings).expect("secure");
        assert!(secure.is_file());
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
    fn oci_layout_builds_index() {
        let scratch = scratch_dir();
        let image = scratch.path().join("oci_demo.tar");
        std::fs::write(&image, b"oci-demo-image-bytes-v1").expect("write image");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let layout = oci_build_layout(
            &image,
            &outdir,
            "registry.example.invalid",
            "demo/app",
            "1.2.3",
        )
        .expect("layout");
        assert!(layout.join("index.json").is_file());
        assert!(layout.join("oci-layout").is_file());
        assert!(layout.join("blobs").join("sha256").is_dir());
    }

    #[test]
    fn octopus_drop_writes_manifest() {
        let scratch = scratch_dir();
        let package = scratch.path().join("release_demo.tar.gz");
        std::fs::write(&package, b"package-bytes-v1").expect("write package");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let drop = octopus_build_drop(
            &package,
            &outdir,
            &OctopusDrop {
                deploy_name: "octopus_demo",
                project: "octopus_demo",
                channel: "Default",
                version: "0.0.0",
                deploy_to: &["Production".to_owned()],
                space: "",
                server: "https://octopus.example.invalid",
            },
        )
        .expect("drop");
        assert!(drop.join("release_demo.tar.gz").is_file());
        assert!(drop.join("would-run.txt").is_file());
    }

    #[test]
    fn promotion_builds_record_and_rollback() {
        let scratch = scratch_dir();
        let artifact = scratch.path().join("release_demo.tar.gz");
        std::fs::write(&artifact, b"promotion-bytes-v1").expect("write artifact");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let promotion = promotion_build(
            &artifact,
            &outdir,
            "promotion_demo",
            "staging",
            "production",
            "1.2.3",
            &[],
        )
        .expect("promotion");
        assert!(promotion.join("promotion.json").is_file());
        assert!(promotion.join("would-run.txt").is_file());
        let rollback =
            promotion_record_rollback(&promotion, "promotion_demo", "1.2.2", "").expect("rollback");
        assert!(rollback.is_file());
        assert_eq!(
            promotion_promote_command("a.tar.gz", "staging", "production", "1.2.3"),
            "promote a.tar.gz staging -> production version 1.2.3"
        );
    }

    #[test]
    fn npm_pack_and_verify_round_trip() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(&src, b"{\"name\": \"npm-demo\", \"version\": \"0.0.0\"}\n")
            .expect("write src");
        let tgz = scratch.path().join("npm_demo.tgz");
        let feed = scratch.path().join("npm_demo.feed.json");
        let digest = npm_create_pack(
            "npm-demo",
            "latest",
            "https://registry.npmjs.org",
            &tgz,
            &feed,
            std::slice::from_ref(&src),
        )
        .expect("pack");
        assert_eq!(digest, sha256_file_hex(&tgz).expect("hex"));
        let (name, tag, registry) = npm_verify_pack(&tgz, &feed).expect("verify");
        assert_eq!(name, "npm-demo");
        assert_eq!(tag, "latest");
        assert_eq!(registry, "https://registry.npmjs.org");
        let members = npm_tar_members(&tgz).expect("members");
        assert_eq!(members, vec!["package/package.json".to_owned()]);
        assert_eq!(
            npm_npmrc_line("registry.npmjs.org", "tok"),
            "//registry.npmjs.org/:_authToken=tok\n"
        );
    }

    #[test]
    fn shlex_split_handles_quotes_escapes_and_spacing() {
        assert_eq!(shlex_split("a b c"), vec!["a", "b", "c"]);
        assert_eq!(shlex_split("  a   b  "), vec!["a", "b"]);
        assert_eq!(shlex_split(""), Vec::<String>::new());
        assert_eq!(shlex_split("   "), Vec::<String>::new());
        assert_eq!(shlex_split("'one two'"), vec!["one two"]);
        assert_eq!(shlex_split("\"one two\""), vec!["one two"]);
        assert_eq!(shlex_split("a\\ b"), vec!["a b"]);
        assert_eq!(shlex_split("\"a\\\"b\""), vec!["a\"b"]);
        assert_eq!(shlex_split("'a\\b'"), vec!["a\\b"]);
        assert_eq!(shlex_split("'unterminated"), vec!["unterminated"]);
        assert_eq!(shlex_split("a\\"), vec!["a"]);
    }

    #[test]
    fn json_and_xml_escape_cover_control_and_markup() {
        assert_eq!(json_escape("a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(json_escape("n\r\nt"), "n\\r\\nt");
        assert_eq!(json_escape("\u{1}"), "\\u0001");
        assert_eq!(json_escape("plain"), "plain");
        assert_eq!(
            xml_escape("<a href=\"x\">&'</a>"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&apos;&lt;/a&gt;"
        );
    }

    #[test]
    fn json_field_helpers_read_escaped_values() {
        let body = r#"{"name": "a\"b\\c\nd", "files": ["x", "y\\z", "w\"q"]}"#;
        assert_eq!(
            json_string_field(body, "name").as_deref(),
            Some("a\"b\\c\nd")
        );
        assert_eq!(json_string_field(body, "missing"), None, "absent key");
        assert_eq!(json_string_field("{\"name\": ", "name"), None);
        assert_eq!(json_string_field("{\"name\": 7}", "name"), None);
        assert_eq!(
            json_files_field(body),
            Some(vec!["x".to_owned(), "y\\z".to_owned(), "w\"q".to_owned()])
        );
        assert_eq!(json_files_field("{}"), None);
        assert_eq!(json_files_field(r#"{"files": 7}"#), None);
    }

    #[test]
    fn command_formatters_cover_optional_segments() {
        assert_eq!(maven_group_path("com.example.demo"), "com/example/demo");
        assert_eq!(
            octopus_push_command("demo-1.0.0", "https://octo.invalid", ""),
            "octo push demo-1.0.0 --server https://octo.invalid"
        );
        assert_eq!(
            octopus_push_command("demo-1.0.0", "https://octo.invalid", "Spaces"),
            "octo push demo-1.0.0 --server https://octo.invalid --space Spaces"
        );
        let release = octopus_release_command(
            "demo",
            "Default",
            "1.0.0",
            "demo-1.0.0",
            &["Production".to_owned(), "Staging".to_owned()],
            "Spaces",
            "https://octo.invalid",
        );
        assert_eq!(
            release,
            "octo create-release --project demo --channel Default --version 1.0.0 --package demo-1.0.0 --deploy-to Production --deploy-to Staging --space Spaces --server https://octo.invalid"
        );
        assert_eq!(
            promotion_rollback_command("demo", "1.0.1"),
            "rollback demo to 1.0.1 (restore the pinned artifact for that version)"
        );
        assert_eq!(
            github_release_command("v1", &[]),
            "gh release create v1 --draft --verify-tag"
        );
    }

    #[test]
    fn maven_file_repo_rejects_empty_coordinates() {
        let scratch = scratch_dir();
        let jar = scratch.path().join("demo.jar");
        std::fs::write(&jar, b"jar").expect("write jar");
        let pom = scratch.path().join("demo.pom");
        std::fs::write(&pom, b"pom").expect("write pom");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        for (group, artifact, version) in [("", "a", "1"), ("g", "", "1"), ("g", "a", "")] {
            let error = maven_build_file_repo(&jar, &pom, &outdir, group, artifact, version)
                .expect_err("empty coordinate");
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        }
    }

    #[test]
    fn stage_helpers_create_and_report_paths() {
        let dir = tempfile_stage_dir("dx-deploy-stage-").expect("stage dir");
        assert!(dir.is_dir());
        assert!(dir.to_string_lossy().contains("dx-deploy-stage-"));
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assert!(tempfile_stage_dir("dx-deploy-stage-").is_ok());
    }

    #[test]
    fn copy_verified_reports_source_digest() {
        let scratch = scratch_dir();
        let src = scratch.path().join("src.bin");
        std::fs::write(&src, b"payload").expect("write src");
        let dest = scratch.path().join("dest.bin");
        std::fs::write(&dest, b"stale-bytes").expect("write dest");
        let digest = copy_verified(&src, &dest).expect("copy");
        assert_eq!(digest, sha256_file_hex(&src).expect("hex"));
        assert_eq!(std::fs::read(&dest).expect("read dest"), b"payload");
    }

    #[test]
    fn which_on_path_finds_present_and_missing_tools() {
        with_env(&[("PATH", Some(""))], || {
            assert!(which_on_path("definitely-not-a-real-tool-xyz").is_none());
        });
        let dir = scratch_dir();
        let tool = dir.path().join("dx_fake_tool");
        std::fs::write(&tool, b"#!/bin/sh\n").expect("write tool");
        let path = format!("{}{}", dir.path().to_string_lossy(), {
            if cfg!(windows) {
                ";"
            } else {
                ":"
            }
        });
        with_env(&[("PATH", Some(&path))], || {
            assert_eq!(
                which_on_path("dx_fake_tool").as_deref(),
                Some(tool.as_path())
            );
            assert!(which_on_path("dx_missing_tool").is_none());
        });
    }

    #[test]
    fn minimal_env_keeps_allowlist_and_applies_extras() {
        with_env(
            &[
                ("DX_TEST_KEEP", Some("kept")),
                ("DX_TEST_DROP", Some("dropped")),
            ],
            || {
                let env = minimal_env(
                    &["DX_TEST_KEEP", "DX_TEST_ABSENT"],
                    &[("DX_TEST_EXTRA", "extra")],
                );
                assert_eq!(env.get("DX_TEST_KEEP").map(String::as_str), Some("kept"));
                assert_eq!(env.get("DX_TEST_EXTRA").map(String::as_str), Some("extra"));
                assert!(!env.contains_key("DX_TEST_DROP"));
                assert!(!env.contains_key("DX_TEST_ABSENT"));
            },
        );
    }

    #[test]
    fn deploy_outdir_prefers_arg_then_env_then_cwd() {
        let scratch = scratch_dir();
        let explicit = scratch.path().join("explicit");
        let argv = vec!["prog".to_owned(), explicit.to_string_lossy().into_owned()];
        assert_eq!(deploy_outdir(&argv), explicit);
        with_env(&[("BUILD_WORKSPACE_DIRECTORY", None)], || {
            let fallback = std::env::current_dir().expect("cwd");
            assert_eq!(deploy_outdir(&["prog".to_owned()]), fallback);
        });
        let from_env = scratch.path().join("from-env");
        with_env(
            &[(
                "BUILD_WORKSPACE_DIRECTORY",
                Some(from_env.to_string_lossy().as_ref()),
            )],
            || {
                assert_eq!(deploy_outdir(&["prog".to_owned()]), from_env);
            },
        );
    }

    #[test]
    fn resolve_runfile_reads_dir_and_manifest() {
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        let payload = runfiles.join("demo/app.bin");
        std::fs::create_dir_all(payload.parent().expect("parent")).expect("nested");
        std::fs::write(&payload, b"payload").expect("write payload");
        let manifest = scratch.path().join("manifest.txt");
        std::fs::write(
            &manifest,
            format!(
                "demo/app.bin {}\nother/missing.bin /nope\nmalformed\n",
                payload.display()
            ),
        )
        .expect("write manifest");

        with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
            ],
            || {
                assert_eq!(resolve_runfile("demo/app.bin").expect("from dir"), payload);
            },
        );
        with_env(
            &[
                (
                    "RUNFILES_DIR",
                    Some(scratch.path().to_string_lossy().as_ref()),
                ),
                (
                    "RUNFILES_MANIFEST_FILE",
                    Some(manifest.to_string_lossy().as_ref()),
                ),
            ],
            || {
                assert_eq!(
                    resolve_runfile("demo/app.bin").expect("from manifest"),
                    payload
                );
            },
        );
        with_env(
            &[("RUNFILES_DIR", None), ("RUNFILES_MANIFEST_FILE", None)],
            || {
                let empty = resolve_runfile("").expect_err("empty rloc");
                assert_eq!(empty.kind(), io::ErrorKind::InvalidInput);
                let missing = resolve_runfile("demo/absent-xyz").expect_err("missing rloc");
                assert_eq!(missing.kind(), io::ErrorKind::NotFound);
            },
        );
    }

    #[test]
    fn npm_find_pack_inputs_reads_manifest_entries() {
        let scratch = scratch_dir();
        let tgz = scratch.path().join("npm_demo.tgz");
        std::fs::write(&tgz, b"tarball-bytes").expect("write tgz");
        let feed = scratch.path().join("npm_demo.feed.json");
        std::fs::write(
            &feed,
            b"{\"name\": \"npm-demo\", \"tarball\": \"npm_demo.tgz\"}\n",
        )
        .expect("write feed");
        let manifest = scratch.path().join("manifest.txt");
        std::fs::write(
            &manifest,
            format!(
                "npm/nested/npm_demo.feed.json {}\nnpm/nested/npm_demo.tgz {}\n",
                feed.display(),
                tgz.display()
            ),
        )
        .expect("write manifest");
        with_env(
            &[
                ("RUNFILES_DIR", None),
                (
                    "RUNFILES_MANIFEST_FILE",
                    Some(manifest.to_string_lossy().as_ref()),
                ),
            ],
            || {
                let (found_feed, found_tgz) = npm_find_pack_inputs().expect("pack pair");
                assert_eq!(found_feed, feed);
                assert_eq!(found_tgz, tgz);
            },
        );
        let empty_dir = scratch.path().join("empty-runfiles");
        std::fs::create_dir_all(&empty_dir).expect("empty runfiles");
        with_env(
            &[
                ("RUNFILES_DIR", Some(empty_dir.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
            ],
            || {
                let error = npm_find_pack_inputs().expect_err("no feed under runfiles");
                assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
            },
        );
    }

    #[test]
    fn npmrc_written_with_owner_only_permissions() {
        let path = npm_write_npmrc("registry.npmjs.org", "tok").expect("npmrc");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read npmrc"),
            "//registry.npmjs.org/:_authToken=tok\n"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&path).expect("stat").permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "npmrc must stay owner-only");
        }
        std::fs::remove_file(&path).expect("cleanup");
    }

    #[test]
    fn promotion_secret_refs_cover_every_ref_kind() {
        let scratch = scratch_dir();
        let secret = scratch.path().join("secret.txt");
        std::fs::write(&secret, b"token").expect("write secret");
        let file_ref = format!("file:{}", secret.display());
        with_env(&[("DX_TEST_PROMOTION_SECRET", None)], || {
            let normalized = promotion_check_secret_refs(&[file_ref.clone()]).expect("file ref");
            assert_eq!(normalized, vec![file_ref.clone()]);
        });
        let missing = promotion_check_secret_refs(&["file:/nope/absent-xyz".to_owned()])
            .expect_err("missing secret file");
        assert_eq!(missing.kind(), io::ErrorKind::NotFound);
        let empty_file =
            promotion_check_secret_refs(&["file:".to_owned()]).expect_err("empty path");
        assert_eq!(empty_file.kind(), io::ErrorKind::NotFound);
        let missing_tool = promotion_check_secret_refs(&["cmd:dx-no-such-tool-xyz".to_owned()])
            .expect_err("missing tool");
        assert_eq!(missing_tool.kind(), io::ErrorKind::NotFound);
        let empty_tool = promotion_check_secret_refs(&["cmd:".to_owned()]).expect_err("empty tool");
        assert_eq!(empty_tool.kind(), io::ErrorKind::NotFound);
        for bad in ["", "has space", "quote'", "dollar$", "back`tick`"] {
            let error = promotion_check_secret_refs(&[bad.to_owned()]).expect_err("invalid ref");
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{bad}");
        }
        let unset = promotion_check_secret_refs(&["DX_TEST_PROMOTION_SECRET".to_owned()])
            .expect_err("unset env ref");
        assert_eq!(unset.kind(), io::ErrorKind::NotFound);
        with_env(&[("DX_TEST_PROMOTION_SECRET", Some("value"))], || {
            assert_eq!(
                promotion_check_secret_refs(&["DX_TEST_PROMOTION_SECRET".to_owned()])
                    .expect("env ref"),
                vec!["DX_TEST_PROMOTION_SECRET".to_owned()]
            );
        });
    }

    #[test]
    fn promotion_health_cmd_rejects_empty_and_missing_tools() {
        let empty = promotion_run_health_cmd("   ").expect_err("empty health cmd");
        assert_eq!(empty.kind(), io::ErrorKind::InvalidInput);
        with_env(&[("PATH", Some(""))], || {
            let missing = promotion_run_health_cmd("dx-no-such-health-tool-xyz")
                .expect_err("missing health tool");
            assert_eq!(missing.kind(), io::ErrorKind::NotFound);
        });
    }

    #[test]
    fn promotion_rollback_requires_directory_and_version() {
        let scratch = scratch_dir();
        let absent = scratch.path().join("absent");
        let missing_dir = promotion_record_rollback(&absent, "demo", "1.0.1", "")
            .expect_err("missing promotion dir");
        assert_eq!(missing_dir.kind(), io::ErrorKind::InvalidInput);
        let artifact = scratch.path().join("demo.tar.gz");
        std::fs::write(&artifact, b"bytes").expect("write artifact");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let promotion = promotion_build(
            &artifact,
            &outdir,
            "demo",
            "staging",
            "production",
            "1.0.2",
            &[],
        )
        .expect("promotion");
        let missing_version =
            promotion_record_rollback(&promotion, "demo", "", "").expect_err("missing version");
        assert_eq!(missing_version.kind(), io::ErrorKind::InvalidInput);
        let pinned = promotion_record_rollback(&promotion, "demo", "1.0.1", "abc123")
            .expect("rollback record");
        let body = std::fs::read_to_string(&pinned).expect("read rollback");
        assert!(body.contains("rollback demo to 1.0.1"));
        assert!(body.contains("expected-sha256: abc123"));
    }

    #[test]
    fn npm_pack_main_requires_full_argv() {
        let short: Vec<String> = ["npm_pack", "a", "b", "c", "d", "e"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(npm_pack_main(&short), 1);
    }

    #[test]
    fn launchers_reject_extra_arguments() {
        let argv = vec!["prog".to_owned(), "out".to_owned(), "extra".to_owned()];
        let spec = OctopusLaunch {
            package_rloc: "demo.tar.gz",
            deploy_name: "demo",
            project: "demo",
            channel: "Default",
            version: "1.0.0",
            deploy_to_raw: "",
            space_default: "",
            url_default: "https://octopus.example.invalid",
        };
        assert_eq!(archive_main("a", "b", "c", &argv), 1);
        assert_eq!(pypi_main("a", "b", "c", "https://pypi.invalid", &argv), 1);
        assert_eq!(
            nuget_main("a", "b", "1.0.0", "https://nuget.invalid", &argv),
            1
        );
        assert_eq!(crates_main("a", "b", "1.0.0", false, &argv), 1);
        assert_eq!(github_main("a", "b", "v1.0.0", &argv), 1);
        assert_eq!(
            maven_main(
                "a",
                "b",
                "com.example",
                "demo",
                "1.0.0",
                "https://maven.invalid",
                &argv
            ),
            1
        );
        assert_eq!(
            oci_main("a", "registry.invalid", "demo/app", "1.0.0", &argv),
            1
        );
        assert_eq!(octopus_main(&spec, &argv), 1);
        assert_eq!(
            promotion_main("a", "b", "staging", "production", "1.0.0", &argv),
            1
        );
    }

    #[test]
    fn launchers_report_missing_pinned_inputs() {
        let outdir = tempfile_stage_dir("dx-deploy-missing-").expect("outdir");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        with_env(
            &[
                ("RUNFILES_DIR", Some(outdir.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
            ],
            || {
                let missing = "dx-absent-pinned-input-xyz";
                assert_eq!(archive_main(missing, missing, missing, &argv), 1);
                assert_eq!(
                    pypi_main(missing, missing, "demo", "https://pypi.invalid", &argv),
                    1
                );
                assert_eq!(
                    nuget_main(missing, "demo", "1.0.0", "https://nuget.invalid", &argv),
                    1
                );
                assert_eq!(crates_main(missing, "demo", "1.0.0", false, &argv), 1);
                assert_eq!(github_main(missing, "demo", "v1.0.0", &argv), 1);
                assert_eq!(
                    maven_main(
                        missing,
                        missing,
                        "com.example",
                        "demo",
                        "1.0.0",
                        "https://maven.invalid",
                        &argv
                    ),
                    1
                );
                assert_eq!(
                    oci_main(missing, "registry.invalid", "demo/app", "1.0.0", &argv),
                    1
                );
                let spec = OctopusLaunch {
                    package_rloc: missing,
                    deploy_name: "demo",
                    project: "demo",
                    channel: "Default",
                    version: "1.0.0",
                    deploy_to_raw: "",
                    space_default: "",
                    url_default: "https://octopus.example.invalid",
                };
                assert_eq!(octopus_main(&spec, &argv), 1);
                assert_eq!(
                    promotion_main(missing, "demo", "staging", "production", "1.0.0", &argv),
                    1
                );
            },
        );
        std::fs::remove_dir_all(&outdir).expect("cleanup");
    }

    #[test]
    fn launchers_refuse_live_publish_without_owner_approval() {
        let outdir = tempfile_stage_dir("dx-deploy-live-").expect("outdir");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        let runfiles = outdir.join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        assert!(runfiles.is_dir());
        let missing = "dx-absent-pinned-input-xyz";
        with_env(
            &[
                (
                    "RUNFILES_DIR",
                    Some(outdir.join("runfiles").to_string_lossy().as_ref()),
                ),
                ("RUNFILES_MANIFEST_FILE", None),
                ("PYPI_PUBLISH_LIVE", Some("1")),
                ("PYPI_API_TOKEN", Some("")),
                ("PYPI_PUBLISH_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(
                    pypi_main(missing, missing, "demo", "https://pypi.invalid", &argv),
                    1,
                    "pypi needs a resolvable wheel before the token gate"
                );
            },
        );
        with_env(
            &[
                ("NUGET_PUBLISH_LIVE", Some("1")),
                ("NUGET_PUBLISH_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(
                    nuget_main(missing, "demo", "0.0.0", "https://nuget.invalid", &argv),
                    1
                );
            },
        );
        with_env(
            &[
                ("GH_RELEASE_LIVE", Some("1")),
                ("GH_RELEASE_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(github_main(missing, "demo", "v0.0.0-dryrun", &argv), 1);
            },
        );
        with_env(
            &[
                ("MAVEN_PUBLISH_LIVE", Some("1")),
                ("MAVEN_PUBLISH_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(
                    maven_main(
                        missing,
                        missing,
                        "com.example",
                        "demo",
                        "0.0.0",
                        "https://maven.invalid",
                        &argv
                    ),
                    1
                );
            },
        );
        with_env(
            &[
                ("OCI_PUBLISH_LIVE", Some("1")),
                ("OCI_PUBLISH_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(
                    oci_main(missing, "registry.invalid", "demo/app", "0.0.0", &argv),
                    1
                );
            },
        );
        with_env(
            &[
                ("CRATES_PUBLISH_LIVE", Some("1")),
                ("CRATES_PUBLISH_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(crates_main(missing, "demo", "0.0.0", false, &argv), 1);
            },
        );
        with_env(
            &[
                ("PROMOTION_LIVE", Some("1")),
                ("PROMOTION_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(
                    promotion_main(missing, "demo", "staging", "production", "0.0.0", &argv),
                    1
                );
            },
        );
        std::fs::remove_dir_all(&outdir).expect("cleanup");
    }

    #[test]
    fn archive_and_promotion_launchers_stage_locally() {
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        let app = runfiles.join("app");
        std::fs::write(&app, b"#!/bin/sh\necho hi\n").expect("write app");
        let tarball = runfiles.join("release_demo.tar.gz");
        let tarball_bytes = archive_bytes(b"payload", "release_demo.tar.gz", false).expect("tar");
        std::fs::write(&tarball, &tarball_bytes).expect("write tarball");
        let digest = sha256_file_hex(&tarball).expect("digest");
        let checksum = runfiles.join("release_demo.tar.gz.sha256");
        std::fs::write(&checksum, format!("{digest}  release_demo.tar.gz\n")).expect("write sum");

        with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                ("DX_PROFILE", Some("dx_dev")),
                ("BUILD_WORKSPACE_DIRECTORY", None),
            ],
            || {
                let outdir = tempfile_stage_dir("dx-deploy-archive-").expect("outdir");
                let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
                assert_eq!(
                    archive_main(
                        "app",
                        "release_demo.tar.gz",
                        "release_demo.tar.gz.sha256",
                        &argv
                    ),
                    0
                );
                assert!(outdir.join("release_demo.tar.gz").is_file());
                std::fs::remove_dir_all(&outdir).expect("cleanup");

                let promotion_out = tempfile_stage_dir("dx-deploy-promotion-").expect("outdir");
                let promotion_argv = vec![
                    "prog".to_owned(),
                    promotion_out.to_string_lossy().into_owned(),
                ];
                assert_eq!(
                    promotion_main(
                        "release_demo.tar.gz",
                        "promotion_demo",
                        "staging",
                        "production",
                        "1.2.3",
                        &promotion_argv
                    ),
                    0
                );
                assert!(promotion_out.join("promotion_demo-promotion").is_dir());
                std::fs::remove_dir_all(&promotion_out).expect("cleanup");
            },
        );
    }

    fn stage_runfiles_inputs(root: &Path) {
        std::fs::create_dir_all(root).expect("runfiles");
        for (name, body) in [
            ("pypi_demo-0.0.0-py3-none-any.whl", &b"wheel"[..]),
            ("pypi_demo-0.0.0.tar.gz", &b"sdist"[..]),
            ("nuget_demo.0.0.0.nupkg", &b"nupkg"[..]),
            ("maven_demo-0.0.0.jar", &b"jar"[..]),
            ("maven_demo-0.0.0.pom", &b"<project/>"[..]),
            ("oci_demo.tar", &b"image"[..]),
            ("release_demo.tar.gz", &b"package"[..]),
            (
                "Cargo.toml",
                &b"[package]\nname = \"crates_demo\"\nversion = \"1.0.0\"\n"[..],
            ),
            (
                "lib.rs",
                &b"pub fn hello() -> &'static str { \"hello\" }\n"[..],
            ),
        ] {
            std::fs::write(root.join(name), body).expect("write runfile input");
        }
    }

    #[test]
    fn launchers_stage_locally_without_publishing() {
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        stage_runfiles_inputs(&runfiles);
        let outdir = tempfile_stage_dir("dx-deploy-stage-").expect("outdir");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                ("DX_PROFILE", Some("dx_dev")),
                ("BUILD_WORKSPACE_DIRECTORY", None),
                ("PYPI_PUBLISH_LIVE", None),
                ("NUGET_PUBLISH_LIVE", None),
                ("CRATES_PUBLISH_LIVE", None),
                ("GH_RELEASE_LIVE", None),
                ("GH_RELEASE_DRY_RUN", Some("1")),
                ("MAVEN_PUBLISH_LIVE", None),
                ("OCI_PUBLISH_LIVE", None),
                ("OCI_PUBLISH_DRY_RUN", Some("1")),
                ("OCTOPUS_PUBLISH_LIVE", None),
            ],
            || {
                assert_eq!(
                    pypi_main(
                        "pypi_demo-0.0.0-py3-none-any.whl",
                        "pypi_demo-0.0.0.tar.gz",
                        "pypi_demo",
                        "https://pypi.invalid",
                        &argv
                    ),
                    0
                );
                assert_eq!(
                    nuget_main(
                        "nuget_demo.0.0.0.nupkg",
                        "nuget_demo",
                        "1.0.0",
                        "https://nuget.invalid",
                        &argv
                    ),
                    0
                );
                assert_eq!(
                    crates_main("Cargo.toml;lib.rs", "crates_demo", "1.0.0", false, &argv),
                    0
                );
                assert_eq!(
                    github_main("release_demo.tar.gz", "github_demo", "v1.0.0", &argv),
                    0
                );
                assert_eq!(
                    maven_main(
                        "maven_demo-0.0.0.jar",
                        "maven_demo-0.0.0.pom",
                        "com.example",
                        "maven_demo",
                        "1.0.0",
                        "https://maven.invalid",
                        &argv
                    ),
                    0
                );
                assert_eq!(
                    oci_main(
                        "oci_demo.tar",
                        "registry.invalid",
                        "demo/app",
                        "1.0.0",
                        &argv
                    ),
                    0
                );
                let spec = OctopusLaunch {
                    package_rloc: "release_demo.tar.gz",
                    deploy_name: "octopus_demo",
                    project: "octopus_demo",
                    channel: "Default",
                    version: "1.0.0",
                    deploy_to_raw: "Production,Staging",
                    space_default: "Spaces",
                    url_default: "https://octopus.example.invalid",
                };
                assert_eq!(octopus_main(&spec, &argv), 0);
            },
        );
        assert!(outdir.join("pypi_demo-wheelhouse").is_dir());
        assert!(outdir.join("maven_demo-repo").is_dir());
        assert!(outdir.join("github_demo-release").is_dir());
        assert!(outdir.join("crates_demo-vendor").is_dir());
        assert!(outdir.join("nuget_demo-feed").is_dir());
        assert!(outdir.join("octopus_demo-drop").is_dir());
        assert!(outdir.join("app-oci-layout").is_dir());
        std::fs::remove_dir_all(&outdir).expect("cleanup");
    }

    #[test]
    fn build_helpers_reject_unusable_inputs() {
        let scratch = scratch_dir();
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let image = scratch.path().join("oci_demo.bin");
        std::fs::write(&image, b"image").expect("write image");
        let error = oci_build_layout(&image, &outdir, "registry.invalid", "demo/app", "1.0.0")
            .expect_err("non-tar image");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);

        let package = scratch.path().join("release_demo.zip");
        std::fs::write(&package, b"package").expect("write package");
        let error = octopus_build_drop(
            &package,
            &outdir,
            &OctopusDrop {
                deploy_name: "octopus_demo",
                project: "octopus_demo",
                channel: "Default",
                version: "1.0.0",
                deploy_to: &[],
                space: "",
                server: "https://octopus.example.invalid",
            },
        )
        .expect_err("non-tar.gz package");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn npm_verify_and_pack_report_feed_problems() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(&src, b"{\"name\": \"npm-demo\", \"version\": \"1.0.0\"}\n")
            .expect("write src");
        let tgz = scratch.path().join("npm_demo.tgz");
        let feed = scratch.path().join("npm_demo.feed.json");
        npm_create_pack(
            "npm-demo",
            "latest",
            "https://registry.npmjs.org",
            &tgz,
            &feed,
            std::slice::from_ref(&src),
        )
        .expect("pack");
        let (name, tag, registry) = npm_verify_pack(&tgz, &feed).expect("verify");
        assert_eq!(name, "npm-demo");
        assert_eq!(tag, "latest");
        assert_eq!(registry, "https://registry.npmjs.org");

        let body = std::fs::read_to_string(&feed).expect("read feed");
        for field in ["name", "tag", "registry", "tarball", "sha256", "files"] {
            let stripped = body
                .lines()
                .filter(|line| !line.contains(&format!("\"{field}\"")))
                .collect::<Vec<_>>()
                .join("\n");
            let path = scratch.path().join(format!("missing_{field}.feed.json"));
            std::fs::write(&path, stripped).expect("write feed");
            let error = npm_verify_pack(&tgz, &path).expect_err(field);
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{field}");
        }

        let wrong_members = scratch.path().join("wrong_members.feed.json");
        std::fs::write(
            &wrong_members,
            body.replace("package/package.json", "package/absent.json"),
        )
        .expect("write feed");
        let error = npm_verify_pack(&tgz, &wrong_members).expect_err("member mismatch");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn npm_pack_main_packs_and_reports_failures() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(&src, b"{\"name\": \"npm-demo\", \"version\": \"1.0.0\"}\n")
            .expect("write src");
        let tgz = scratch.path().join("npm_demo.tgz");
        let feed = scratch.path().join("npm_demo.feed.json");
        let argv = vec![
            "npm_pack".to_owned(),
            "npm-demo".to_owned(),
            "latest".to_owned(),
            "https://registry.npmjs.org".to_owned(),
            tgz.to_string_lossy().into_owned(),
            feed.to_string_lossy().into_owned(),
            src.to_string_lossy().into_owned(),
        ];
        assert_eq!(npm_pack_main(&argv), 0);
        assert!(tgz.is_file());
        assert!(feed.is_file());
        let mut missing_src = argv.clone();
        missing_src.pop();
        missing_src.push(
            scratch
                .path()
                .join("absent.json")
                .to_string_lossy()
                .into_owned(),
        );
        assert_eq!(npm_pack_main(&missing_src), 1);
    }

    #[test]
    fn npm_deploy_main_releases_a_staged_pack() {
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        let tgz = runfiles.join("npm_demo.tgz");
        let feed = runfiles.join("npm_demo.feed.json");
        let src = runfiles.join("package.json");
        std::fs::write(&src, b"{\"name\": \"npm-demo\", \"version\": \"1.0.0\"}\n")
            .expect("write src");
        npm_create_pack(
            "npm-demo",
            "latest",
            "https://registry.npmjs.org",
            &tgz,
            &feed,
            std::slice::from_ref(&src),
        )
        .expect("pack");
        let outdir = tempfile_stage_dir("dx-deploy-npm-").expect("outdir");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                ("DX_PROFILE", Some("dx_dev")),
                ("BUILD_WORKSPACE_DIRECTORY", None),
                ("NPM_PUBLISH_LIVE", None),
            ],
            || {
                assert_eq!(npm_deploy_main(&argv), 0);
            },
        );
        assert!(outdir.join("npm_demo.tgz").is_file());
        assert!(outdir
            .join("npm_demo-feed")
            .join("npm_demo.feed.json")
            .is_file());
        std::fs::remove_dir_all(&outdir).expect("cleanup");
    }
}
