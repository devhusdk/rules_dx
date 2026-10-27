#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

mod cloudflare;
pub mod http;
mod maven;
mod run;
#[cfg(test)]
pub(crate) mod test_support;

pub use cloudflare::*;
pub use maven::*;
pub use run::{deploy_profile, DeployRun};

use run::{
    env_or_empty, scrubbed_command, OwnerGate, DOTNET_CHILD_ENV_EXTRA, PUBLISH_CHILD_ENV_KEEP,
};

use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::Path;

const RECORDSIZE: usize = 20 * 512;
const MTIME: u64 = 0;
const UID: u64 = 0;
const GID: u64 = 0;

const USTAR_LIMIT: &str =
    "must be at most 100 bytes, or split at a '/' into a prefix of at most 155 bytes and a name of at most 100 bytes, with no NUL";

/// Returns whether a path fits the ustar name and prefix fields.
fn fits_ustar(name: &str) -> bool {
    name.len() <= 100
        || name
            .match_indices('/')
            .any(|(at, _)| at <= 155 && name.len() - at - 1 <= 100)
}

/// Returns one ustar header whose numeric fields use the python tarfile encoding.
fn ustar_header(name: &str, size: u64, mode: u32) -> io::Result<tar::Header> {
    if name.is_empty() || name.contains('\0') || !fits_ustar(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("tar member name {name:?} {USTAR_LIMIT}"),
        ));
    }
    let mut header = tar::Header::new_ustar();
    header.set_entry_type(tar::EntryType::Regular);
    header.set_mode(mode & 0o7777);
    header.set_uid(UID);
    header.set_gid(GID);
    header.set_size(size);
    header.set_mtime(MTIME);
    header.set_path(name)?;
    header.set_cksum();
    let field = format!("{:06o}\0 ", header.cksum()?);
    header.as_old_mut().cksum.copy_from_slice(field.as_bytes());
    Ok(header)
}

/// Returns one ustar image padded to a whole record.
fn tar_archive<'a>(members: impl Iterator<Item = (&'a str, &'a [u8], u32)>) -> io::Result<Vec<u8>> {
    let mut builder = tar::Builder::new(Vec::new());
    for (name, data, mode) in members {
        let header = ustar_header(name, data.len() as u64, mode)?;
        builder.append(&header, data)?;
    }
    let mut image = builder.into_inner()?;
    image.resize(image.len().div_ceil(RECORDSIZE) * RECORDSIZE, 0);
    Ok(image)
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

pub fn member_name_error(name: &str) -> Option<String> {
    name_error(name, false)
}

/// Returns why a member name is unsafe, optionally allowing '='.
fn name_error(name: &str, allow_equals: bool) -> Option<String> {
    if name.is_empty() {
        return Some("archive member name must not be empty".to_owned());
    }
    if name.contains('\0') || !fits_ustar(name) {
        return Some(format!("archive member name {name:?} {USTAR_LIMIT}"));
    }
    if name.starts_with('/') {
        return Some(format!("archive member name {name:?} must be relative"));
    }
    if name.contains('\\') {
        return Some(format!(
            "archive member name {name:?} must use '/' separators"
        ));
    }
    if !allow_equals && name.contains('=') {
        return Some(format!("archive member name {name:?} must not contain '='"));
    }
    for segment in name.split('/') {
        if segment.is_empty() {
            return Some(format!(
                "archive member name {name:?} must not contain empty segments"
            ));
        }
        if segment == "." || segment == ".." {
            return Some(format!(
                "archive member name {name:?} must not contain '.' or '..'"
            ));
        }
    }
    None
}

/// Parses `[--executable <member>]... <name=path>...` into archive members.
pub fn member_args(args: &[String]) -> Result<Vec<(String, std::path::PathBuf, bool)>, String> {
    let mut executables: Vec<&str> = Vec::new();
    let mut rest = args;
    while let [flag, tail @ ..] = rest {
        if flag != "--executable" {
            break;
        }
        let [name, after @ ..] = tail else {
            return Err("--executable needs a member name".to_owned());
        };
        executables.push(name);
        rest = after;
    }
    if rest.is_empty() {
        return Err("need at least one <name=path> member".to_owned());
    }
    let mut members = Vec::with_capacity(rest.len());
    for pair in rest {
        let Some((name, path)) = pair.split_once('=') else {
            return Err(format!("member {pair:?} must be <name=path>"));
        };
        members.push((
            name.to_owned(),
            std::path::PathBuf::from(path),
            executables.contains(&name),
        ));
    }
    if let Some(name) = executables
        .iter()
        .find(|name| !members.iter().any(|member| member.0 == **name))
    {
        return Err(format!("--executable {name:?} names no member"));
    }
    Ok(members)
}

/// Writes one deterministic archive of files; the dst suffix picks tar.gz or zip.
pub fn archive_members(members: &[(&str, &Path, bool)], dst: &Path) -> io::Result<()> {
    if members.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "archive members: need at least one member",
        ));
    }
    let mut staged: Vec<(&str, Vec<u8>, u32)> = Vec::with_capacity(members.len());
    for (name, src, executable) in members {
        if let Some(reason) = member_name_error(name) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, reason));
        }
        let mode = if *executable { 0o755 } else { 0o644 };
        staged.push((name, std::fs::read(src)?, mode));
    }
    let refs: Vec<(&str, &[u8], u32)> = staged
        .iter()
        .map(|staged| (staged.0, staged.1.as_slice(), staged.2))
        .collect();
    archive_tree(&refs, dst)
}

/// Writes one deterministic archive of in-memory members sorted by name; a .zip dst writes zip.
pub fn archive_tree(members: &[(&str, &[u8], u32)], dst: &Path) -> io::Result<()> {
    if members.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "archive members: need at least one member",
        ));
    }
    let mut ordered: Vec<(&str, &[u8], u32)> = members.to_vec();
    ordered.sort_by(|left, right| left.0.cmp(right.0));
    for pair in ordered.windows(2) {
        if pair[0].0 == pair[1].0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("archive members: duplicate name {:?}", pair[0].0),
            ));
        }
    }
    for (name, _, _) in &ordered {
        if let Some(reason) = name_error(name, true) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, reason));
        }
    }
    let image = if dst.extension().is_some_and(|ext| ext == "zip") {
        zip_archive(&ordered)?
    } else {
        gzip_compress(&tar_archive(ordered.into_iter())?)?
    };
    std::fs::write(dst, image)?;
    Ok(())
}

/// Returns one zip image with a fixed DOS time, unix modes on every host and a fixed Deflate level.
fn zip_archive(members: &[(&str, &[u8], u32)]) -> io::Result<Vec<u8>> {
    let mut writer = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
    for (name, data, mode) in members {
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(9))
            .last_modified_time(zip::DateTime::default())
            .unix_permissions(mode & 0o7777)
            .system(zip::System::Unix)
            .large_file(false);
        writer
            .start_file(*name, options)
            .map_err(io::Error::other)?;
        writer.write_all(data)?;
    }
    Ok(writer.finish().map_err(io::Error::other)?.into_inner())
}

fn listing_error(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn is_zip(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "zip")
}

/// Checks one member's type and uniqueness, then hands it to the visitor.
fn archive_entry<T>(
    seen: &mut std::collections::HashSet<String>,
    name: String,
    regular: bool,
    mode: u32,
    body: &mut dyn Read,
    visit: &mut impl FnMut(String, u32, &mut dyn Read) -> io::Result<Option<T>>,
) -> io::Result<Option<T>> {
    if !regular {
        return Err(listing_error(format!(
            "archive listing: member {name:?} is not a regular file"
        )));
    }
    if !seen.insert(name.clone()) {
        return Err(listing_error(format!(
            "archive listing: duplicate member {name:?}"
        )));
    }
    visit(name, mode, body)
}

fn archive_entries<T>(
    path: &Path,
    mut visit: impl FnMut(String, u32, &mut dyn Read) -> io::Result<Option<T>>,
) -> io::Result<Vec<T>> {
    let file = std::fs::File::open(path)?;
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    if is_zip(path) {
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|error| listing_error(format!("archive listing: {error}")))?;
        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|error| listing_error(format!("archive listing: {error}")))?;
            let name = entry
                .name()
                .map_err(|error| listing_error(format!("archive listing: {error}")))?
                .into_owned();
            let unix = entry.system() == zip::System::Unix;
            let Some(mode) = entry.unix_mode().filter(|_| unix) else {
                return Err(listing_error(format!(
                    "archive listing: member {name:?} has no unix mode"
                )));
            };
            let regular = entry.is_file() && !entry.is_symlink();
            if let Some(item) = archive_entry(
                &mut seen,
                name,
                regular,
                mode & 0o7777,
                &mut entry,
                &mut visit,
            )? {
                out.push(item);
            }
        }
        return Ok(out);
    }
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    for entry in archive.entries()? {
        let mut entry = entry?;
        let name =
            entry.path()?.to_str().map(str::to_owned).ok_or_else(|| {
                listing_error("archive listing: member name is not UTF-8".to_owned())
            })?;
        let regular = entry.header().entry_type() == tar::EntryType::Regular;
        let mode = if regular { entry.header().mode()? } else { 0 };
        if let Some(item) = archive_entry(&mut seen, name, regular, mode, &mut entry, &mut visit)? {
            out.push(item);
        }
    }
    io::copy(&mut archive.into_inner(), &mut io::sink())?;
    Ok(out)
}

/// Lists name, mode and sha256 of every regular member of a tar.gz or zip.
pub fn archive_listing(path: &Path) -> io::Result<Vec<(String, u32, String)>> {
    archive_entries(path, |name, mode, body| {
        Ok(Some((name, mode, dx_digest::sha256_reader_hex(body)?)))
    })
}

/// Returns the bytes of one named member of a tar.gz or zip.
pub fn archive_read_member(path: &Path, member: &str) -> io::Result<Vec<u8>> {
    let mut found = archive_entries(path, |name, _, body| {
        if name != member {
            return Ok(None);
        }
        let mut data = Vec::new();
        body.read_to_end(&mut data)?;
        Ok(Some(data))
    })?;
    found.pop().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("archive has no member {member:?}"),
        )
    })
}

/// Extracts every regular member of a tar.gz or zip into a new dir and lists name, mode and sha256.
pub fn archive_extract(path: &Path, dir: &Path) -> io::Result<Vec<(String, u32, String)>> {
    std::fs::create_dir_all(dir.parent().unwrap_or(dir))?;
    std::fs::create_dir(dir)?;
    archive_entries(path, |name, mode, body| {
        if let Some(reason) = name_error(&name, true) {
            return Err(listing_error(format!("archive extract: {reason}")));
        }
        if name.contains(':') {
            return Err(listing_error(format!(
                "archive extract: archive member name {name:?} must not contain ':'"
            )));
        }
        let target = dir.join(&name);
        std::fs::create_dir_all(target.parent().unwrap_or(dir))?;
        let mut data = Vec::new();
        body.read_to_end(&mut data)?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)?;
        file.write_all(&data)?;
        drop(file);
        set_member_mode(&target, mode)?;
        Ok(Some((name, mode, dx_digest::sha256_hex(&data))))
    })
}

#[cfg(unix)]
fn set_member_mode(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode & 0o7777))
}

#[cfg(not(unix))]
fn set_member_mode(_path: &Path, _mode: u32) -> io::Result<()> {
    Ok(())
}

/// Archives one executable under its basename.
pub fn archive_file(src: &Path, dst: &Path) -> io::Result<()> {
    let name = basename(src)?;
    archive_members(&[(name.as_str(), src, true)], dst)
}

pub fn archive_bytes(data: &[u8], name: &str, executable: bool) -> io::Result<Vec<u8>> {
    let mode = if executable { 0o755 } else { 0o644 };
    let tar = tar_archive([(name, data, mode)].into_iter())?;
    gzip_compress(&tar)
}

/// Writes one deterministic docker-save archive and returns its config digest.
pub fn archive_image(
    layers: &[&Path],
    platform: &str,
    entrypoint: &[&str],
    dst: &Path,
) -> io::Result<String> {
    if layers.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "archive image: need at least one layer",
        ));
    }
    let (os, architecture) = match platform {
        "linux/amd64" => ("linux", "amd64"),
        "linux/arm64" => ("linux", "arm64"),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "archive image: want platform linux/amd64 or linux/arm64, got '{platform}'"
                ),
            ));
        }
    };
    let mut blobs: Vec<(String, Vec<u8>)> = Vec::new();
    let mut layer_names = Vec::with_capacity(layers.len());
    let mut diff_ids = Vec::with_capacity(layers.len());
    for layer in layers {
        let bytes = std::fs::read(layer)?;
        let digest = dx_digest::sha256_hex(&bytes);
        let diff_id = match oci_compression(&bytes) {
            OciCompression::Gzip => {
                dx_digest::sha256_reader_hex(flate2::read::MultiGzDecoder::new(bytes.as_slice()))
                    .map_err(|error| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!(
                                "archive image: layer {} is not valid gzip: {error}",
                                layer.display()
                            ),
                        )
                    })?
            }
            OciCompression::Zstd => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "archive image: layer {} uses unsupported layer compression zstd; want uncompressed or gzip",
                        layer.display()
                    ),
                ));
            }
            OciCompression::None => digest.clone(),
        };
        let name = format!("blobs/sha256/{digest}");
        if !blobs.iter().any(|(seen, _)| *seen == name) {
            blobs.push((name.clone(), bytes));
        }
        layer_names.push(name);
        diff_ids.push(format!("sha256:{diff_id}"));
    }
    let config = serde_json::to_string(&serde_json::json!({
        "architecture": architecture,
        "config": {"Entrypoint": entrypoint},
        "created": "1970-01-01T00:00:00Z",
        "os": os,
        "rootfs": {"diff_ids": diff_ids, "type": "layers"},
    }))?;
    let config_hex = dx_digest::sha256_hex(config.as_bytes());
    let config_name = format!("blobs/sha256/{config_hex}");
    let manifest = serde_json::to_string(&serde_json::json!([{
        "Config": config_name,
        "Layers": layer_names,
        "RepoTags": [],
    }]))?;
    let members = [
        (config_name.as_str(), config.as_bytes(), 0o644),
        ("manifest.json", manifest.as_bytes(), 0o644),
    ]
    .into_iter()
    .chain(
        blobs
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.as_slice(), 0o644)),
    );
    std::fs::write(dst, tar_archive(members)?)?;
    Ok(format!("sha256:{config_hex}"))
}

pub fn hash_file(src: &Path, dst: &Path) -> io::Result<()> {
    let base = basename(src)?;
    let line = format!("{}  {base}\n", dx_digest::sha256_file_hex(src)?);
    std::fs::write(dst, line.as_bytes())?;
    Ok(())
}

pub fn bin_usage(prog: &str, usage: &str) -> i32 {
    eprintln!("usage: {prog} {usage}");
    1
}

pub fn bin_cannot(prog: &str, action: &str, target: &Path, error: impl std::fmt::Display) -> i32 {
    eprintln!("{prog}: cannot {action} {}: {error}", target.display());
    1
}

/// Copies one file and verifies byte identity.
pub fn copy_verified(src: &Path, dest: &Path) -> io::Result<String> {
    dx_atomic_fs::copy_atomic(src, dest)?;
    let want = dx_digest::sha256_file_hex(src)?;
    let got = dx_digest::sha256_file_hex(dest)?;
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

/// Resolves one runfiles rlocation through the Bazel runfiles library.
pub fn resolve_runfile(rloc: &str) -> io::Result<std::path::PathBuf> {
    if rloc.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "empty runfiles rlocation",
        ));
    }
    let runfiles = runfiles::Runfiles::create().map_err(|error| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("cannot open runfiles for '{rloc}': {error}"),
        )
    })?;
    match runfiles.rlocation_from(rloc, "") {
        Some(path) if path.exists() => Ok(path),
        _ => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("runfile not found for '{rloc}'"),
        )),
    }
}

/// Returns the deploy output directory.
pub fn deploy_outdir(argv: &[String]) -> std::path::PathBuf {
    if argv.len() >= 2 {
        return absolute_from_invocation(Path::new(&argv[1]));
    }
    std::env::var("BUILD_WORKSPACE_DIRECTORY")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        })
}

/// Resolves a relative path against the directory the user ran bazel from.
fn absolute_from_invocation(path: &Path) -> std::path::PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    let base = std::env::var_os("BUILD_WORKING_DIRECTORY")
        .filter(|dir| !dir.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    base.join(path)
}

/// Serializes one JSON document pretty-printed with a trailing newline.
fn render_json(value: &serde_json::Value) -> std::io::Result<String> {
    Ok(serde_json::to_string_pretty(value)? + "\n")
}

/// Writes one owner-only file.
pub fn write_secure_file(
    directory: &Path,
    name: &str,
    content: &str,
) -> io::Result<std::path::PathBuf> {
    let path = directory.join(name);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(&path)?;
    file.write_all(content.as_bytes())?;
    Ok(path)
}

/// Stages one release tarball pair.
pub fn archive_stage_release(
    app_src: &Path,
    tarball_src: &Path,
    checksum_src: &Path,
    outdir: &Path,
) -> io::Result<(String, std::path::PathBuf, std::path::PathBuf)> {
    let app_name = std::fs::read_to_string(app_src)?.trim().to_owned();
    if app_name.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("archive_deploy: {} names no app", app_src.display()),
        ));
    }
    let text = std::fs::read_to_string(checksum_src)?;
    let expected = text
        .split_whitespace()
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "empty checksum file"))?;
    let actual = dx_digest::sha256_file_hex(tarball_src)?;
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
    dx_atomic_fs::copy_atomic(tarball_src, &tarball_dest)?;
    dx_atomic_fs::copy_atomic(checksum_src, &checksum_dest)?;
    Ok((app_name, tarball_dest, checksum_dest))
}

/// Runs the archive deploy launcher.
pub fn archive_main(
    app_rloc: &str,
    tarball_rloc: &str,
    checksum_rloc: &str,
    argv: &[String],
) -> i32 {
    let Some(run) = DeployRun::start(
        "archive_deploy",
        "the release is exactly the files pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let outdir = run.outdir();
    let resolved = (|| -> io::Result<(String, std::path::PathBuf, std::path::PathBuf)> {
        let app = resolve_runfile(app_rloc)?;
        let tarball = resolve_runfile(tarball_rloc)?;
        let checksum = resolve_runfile(checksum_rloc)?;
        archive_stage_release(&app, &tarball, &checksum, outdir)
    })();
    match resolved {
        Ok((app_name, tarball_dest, checksum_dest)) => {
            let profile = deploy_profile();
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

/// Builds one PyPI wheelhouse.
pub fn pypi_build_wheelhouse(
    wheel_src: &Path,
    sdist_src: Option<&Path>,
    outdir: &Path,
    dist_name: &str,
) -> io::Result<std::path::PathBuf> {
    let invalid = |message: String| io::Error::new(io::ErrorKind::InvalidData, message);
    let (project, version) = pypi_artifact_identity(&basename(wheel_src)?).map_err(invalid)?;
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
const PYPI_GATE: OwnerGate = OwnerGate {
    tool: "pypi_deploy",
    action: "upload",
    credentials: &["PYPI_API_TOKEN"],
    approved: "PYPI_PUBLISH_APPROVED",
    live: "PYPI_PUBLISH_LIVE",
};

/// Extra env names twine keeps beyond the publish base list.
const TWINE_CHILD_ENV_EXTRA: &[&str] = &["REQUESTS_CA_BUNDLE"];

/// Runs the PyPI deploy launcher.
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
        let mut files = vec![wheel.clone()];
        if let Some(sdist) = &sdist {
            files.push(sdist.clone());
        }
        let mut cmd = scrubbed_command(
            Path::new("twine"),
            &[PUBLISH_CHILD_ENV_KEEP, TWINE_CHILD_ENV_EXTRA],
            &[
                ("TWINE_USERNAME", "__token__"),
                ("TWINE_PASSWORD", secrets.get("PYPI_API_TOKEN")),
            ],
        );
        cmd.arg("upload")
            .arg("--non-interactive")
            .arg("--repository-url")
            .arg(repository_url);
        for file in &files {
            cmd.arg(file);
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
const NUGET_GATE: OwnerGate = OwnerGate {
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

fn package_relative_paths(
    crate_files: &[std::path::PathBuf],
    manifest_name: &str,
    label: &str,
) -> io::Result<Vec<(std::path::PathBuf, String)>> {
    let bad = |message: String| io::Error::new(io::ErrorKind::InvalidInput, message);
    if crate_files.is_empty() {
        return Err(bad(format!("{label}: need at least one source file")));
    }
    let mut manifests: Vec<&std::path::PathBuf> = crate_files
        .iter()
        .filter(|src| src.file_name().and_then(|name| name.to_str()) == Some(manifest_name))
        .collect();
    manifests.sort_by_key(|src| src.components().count());
    let Some(first) = manifests.first() else {
        return Err(bad(format!(
            "{label}: no {manifest_name} among the source files"
        )));
    };
    if let Some(second) = manifests.get(1) {
        if second.components().count() == first.components().count() {
            return Err(bad(format!(
                "{label}: ambiguous package root, found '{}' and '{}'",
                first.display(),
                second.display()
            )));
        }
    }
    let root = first.parent().unwrap_or_else(|| Path::new(""));
    let mut mapped: Vec<(std::path::PathBuf, String)> = Vec::new();
    for src in crate_files {
        let rel = src.strip_prefix(root).map_err(|_| {
            bad(format!(
                "{label}: '{}' is outside the package root '{}'",
                src.display(),
                root.display()
            ))
        })?;
        let mut parts: Vec<&str> = Vec::new();
        for component in rel.components() {
            match component {
                std::path::Component::Normal(part) => match part.to_str() {
                    Some(part) if !part.is_empty() => parts.push(part),
                    _ => return Err(bad(format!("{label}: non-UTF-8 path '{}'", src.display()))),
                },
                _ => {
                    return Err(bad(format!(
                        "{label}: unsafe package path '{}'",
                        rel.display()
                    )))
                }
            }
        }
        if parts.is_empty() {
            return Err(bad(format!(
                "{label}: empty package path for '{}'",
                src.display()
            )));
        }
        mapped.push((src.clone(), parts.join("/")));
    }
    mapped.sort_by(|left, right| left.1.cmp(&right.1));
    for pair in mapped.windows(2) {
        let (left, right) = (&pair[0].1, &pair[1].1);
        if left == right {
            return Err(bad(format!("{label}: path collision on '{left}'")));
        }
        if right.starts_with(&format!("{left}/")) {
            return Err(bad(format!(
                "{label}: '{left}' is both a file and a directory"
            )));
        }
    }
    Ok(mapped)
}

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

/// Maps npm input files to package-relative paths under the package.json directory.
pub fn npm_package_paths(
    srcs: &[std::path::PathBuf],
) -> io::Result<Vec<(std::path::PathBuf, String)>> {
    package_relative_paths(srcs, "package.json", "npm pack")
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
const CRATES_GATE: OwnerGate = OwnerGate {
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

fn create_private_dir(path: &Path) -> io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder.create(path)
}

fn tempfile_stage_dir(prefix: &str) -> io::Result<std::path::PathBuf> {
    for counter in 0..100 {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("{prefix}{}-{nanos}-{counter}", std::process::id()));
        match create_private_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "cannot stage tempdir",
    ))
}

/// Owner-only temp directory that is removed when dropped, including on panic.
struct PrivateStageDir(std::path::PathBuf);

impl PrivateStageDir {
    fn new(prefix: &str) -> io::Result<Self> {
        tempfile_stage_dir(prefix).map(Self)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for PrivateStageDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
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

/// Returns the asset names, rejecting two assets that share one.
fn github_asset_bases(srcs: &[&std::path::PathBuf]) -> io::Result<Vec<String>> {
    let mut seen: std::collections::HashMap<String, &Path> = std::collections::HashMap::new();
    let mut bases = Vec::new();
    for src in srcs {
        let base = basename(src)?;
        if let Some(first) = seen.get(&base) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "assets {} and {} share the asset name '{base}'",
                    first.display(),
                    src.display()
                ),
            ));
        }
        seen.insert(base.clone(), src.as_path());
        bases.push(base);
    }
    Ok(bases)
}

/// Builds one GitHub release staging dir.
pub fn github_build_staging(
    asset_srcs: &[std::path::PathBuf],
    outdir: &Path,
    deploy_name: &str,
    tag: &str,
) -> io::Result<std::path::PathBuf> {
    let mut sorted: Vec<&std::path::PathBuf> = asset_srcs.iter().collect();
    sorted.sort();
    let bases = github_asset_bases(&sorted)?;
    let staging = outdir.join(deploy_name.to_owned() + "-release");
    std::fs::create_dir_all(&staging)?;
    let mut lines = vec![String::from("# would-run manifest for github_deploy")];
    for (src, base) in sorted.iter().zip(&bases) {
        let dest = staging.join(base);
        let digest = copy_verified(src, &dest)?;
        lines.push(format!("asset-sha256: {digest}  {base}"));
    }
    lines.push(github_release_command(tag, &bases));
    std::fs::write(staging.join("would-run.txt"), lines.join("\n").as_bytes())?;
    Ok(staging)
}

/// Owner approval gate for github_deploy.
const GITHUB_GATE: OwnerGate = OwnerGate {
    tool: "github_deploy",
    action: "publish",
    credentials: &[],
    approved: "GH_RELEASE_APPROVED",
    live: "GH_RELEASE_LIVE",
};

/// Extra env names `gh` keeps: its auth, host, config and session bus.
const GH_CHILD_ENV_EXTRA: &[&str] = &[
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
    "GH_HOST",
    "GH_REPO",
    "GH_CONFIG_DIR",
    "GH_PROMPT_DISABLED",
    "XDG_CONFIG_HOME",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
];

/// Runs the GitHub deploy launcher.
pub fn github_main(asset_rlocs: &str, deploy_name: &str, tag: &str, argv: &[String]) -> i32 {
    let Some(run) = DeployRun::start(
        "github_deploy",
        "the release is exactly the artifacts pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let outdir = run.outdir();
    let mut assets = Vec::new();
    for rloc in asset_rlocs.split(';').filter(|s| !s.is_empty()) {
        let Some(path) = run.resolve(rloc) else {
            return 1;
        };
        assets.push(path);
    }
    let mut sorted: Vec<&std::path::PathBuf> = assets.iter().collect();
    sorted.sort();
    let bases = match github_asset_bases(&sorted) {
        Ok(bases) => bases,
        Err(error) => {
            eprintln!("github_deploy: {error}");
            return 1;
        }
    };
    if GITHUB_GATE.requested(env_or_empty) {
        if tag == "v0.0.0-dryrun" {
            eprintln!(
                "github_deploy: live publish refuses placeholder tag v0.0.0-dryrun; push a real tag first"
            );
            return 1;
        }
        if let Some(error) = GITHUB_GATE.refusal(env_or_empty) {
            eprintln!("{error}");
            return 1;
        }
        if which_on_path("gh").is_none() {
            eprintln!("github_deploy: 'gh' CLI not found on PATH");
            return 1;
        }
        let mut cmd = scrubbed_command(
            Path::new("gh"),
            &[PUBLISH_CHILD_ENV_KEEP, GH_CHILD_ENV_EXTRA],
            &[],
        );
        cmd.arg("release").arg("create").arg(tag);
        for src in &assets {
            match std::path::absolute(src) {
                Ok(path) => cmd.arg(path),
                Err(error) => {
                    eprintln!("github_deploy: cannot resolve {}: {error}", src.display());
                    return 1;
                }
            };
        }
        cmd.arg("--draft").arg("--verify-tag");
        if let Some(workspace) =
            std::env::var_os("BUILD_WORKSPACE_DIRECTORY").filter(|dir| !dir.is_empty())
        {
            cmd.current_dir(workspace);
        }
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
    match github_build_staging(&assets, outdir, deploy_name, tag) {
        Ok(staging) => {
            if std::env::var("GH_RELEASE_DRY_RUN").unwrap_or_default() == "1" {
                println!("github_deploy: dry run (GH_RELEASE_DRY_RUN=1); would create a draft release, publishing nothing:");
            } else {
                println!("github_deploy: staged draft release, publishing nothing:");
            }
            println!("  tag: {tag}");
            for (src, base) in sorted.iter().zip(&bases) {
                println!("  asset: {base} ({})", src.display());
            }
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
        for suffix in dx_path::host::EXECUTABLE_SUFFIXES {
            let exe = std::path::Path::new(dir).join(format!("{tool}{suffix}"));
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

/// Maps the platform to its os and architecture and checks the image suffix.
fn oci_check_source(image: &Path, platform: &str) -> io::Result<(&'static str, &'static str)> {
    let (os, architecture) = match platform {
        "linux/amd64" => ("linux", "amd64"),
        "linux/arm64" => ("linux", "arm64"),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("oci image: want platform linux/amd64 or linux/arm64, got '{platform}'"),
            ));
        }
    };
    let src_str = image.to_string_lossy();
    if !(src_str.ends_with(".tar") || src_str.ends_with(".tar.gz")) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("oci image: want exactly one .tar source, got '{src_str}'"),
        ));
    }
    Ok((os, architecture))
}

/// Builds one OCI image layout from an image archive.
pub fn oci_build_layout(
    image_tar_src: &Path,
    outdir: &Path,
    registry: &str,
    repository: &str,
    tag: &str,
    platform: &str,
) -> io::Result<std::path::PathBuf> {
    let (os, architecture) = oci_check_source(image_tar_src, platform)?;
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
    let base = repository.rsplit('/').next().unwrap_or(repository);
    let layout = outdir.join(base.to_owned() + "-oci-layout");
    let staging = outdir.join(base.to_owned() + "-oci-layout.tmp");
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    let reference = format!("{registry}/{repository}:{tag}");
    let staged = oci_stage_layout(image_tar_src, &staging, &reference, os, architecture);
    if let Err(error) = staged {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    if layout.exists() {
        std::fs::remove_dir_all(&layout)?;
    }
    if let Err(error) = std::fs::rename(&staging, &layout) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    Ok(layout)
}

/// Checks that the image config matches the declared os and architecture.
fn oci_check_config(archive: &OciArchive, os: &str, architecture: &str) -> io::Result<()> {
    let config: serde_json::Value = serde_json::from_slice(&archive.config).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("oci image: malformed image config: {error}"),
        )
    })?;
    let found_os = config["os"].as_str().unwrap_or("");
    let found_architecture = config["architecture"].as_str().unwrap_or("");
    if found_os != os || found_architecture != architecture {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "oci image: image config is '{found_os}/{found_architecture}', want platform '{os}/{architecture}'"
            ),
        ));
    }
    Ok(())
}

/// Writes the OCI layout for one image archive into a staging dir.
fn oci_stage_layout(
    image: &Path,
    staging: &Path,
    reference: &str,
    os: &str,
    architecture: &str,
) -> io::Result<()> {
    let blobs = staging.join("blobs").join("sha256");
    std::fs::create_dir_all(&blobs)?;
    let archive = oci_read_archive(image, Some(&blobs)).map_err(|error| {
        io::Error::new(io::ErrorKind::InvalidData, format!("oci layout: {error}"))
    })?;
    oci_check_config(&archive, os, architecture)?;
    std::fs::write(
        blobs.join(archive.config_digest.trim_start_matches("sha256:")),
        &archive.config,
    )?;
    let layers: Vec<serde_json::Value> = archive
        .layers
        .iter()
        .map(|layer| {
            serde_json::json!({
                "digest": layer.digest,
                "mediaType": layer.media_type,
                "size": layer.size,
            })
        })
        .collect();
    let manifest_bytes = serde_json::to_string(&serde_json::json!({
        "config": {
            "digest": archive.config_digest,
            "mediaType": "application/vnd.oci.image.config.v1+json",
            "size": archive.config.len(),
        },
        "layers": layers,
        "mediaType": "application/vnd.oci.image.manifest.v1+json",
        "schemaVersion": 2,
    }))?;
    let manifest_digest = format!(
        "sha256:{}",
        dx_digest::sha256_hex(manifest_bytes.as_bytes())
    );
    std::fs::write(
        blobs.join(manifest_digest.replace("sha256:", "")),
        manifest_bytes.as_bytes(),
    )?;
    let index_body = serde_json::to_string(&serde_json::json!({
        "manifests": [{
            "annotations": {"org.opencontainers.image.ref.name": reference},
            "digest": manifest_digest,
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "size": manifest_bytes.len(),
        }],
        "mediaType": "application/vnd.oci.image.index.v1+json",
        "schemaVersion": 2,
    }))?;
    std::fs::write(staging.join("index.json"), index_body.as_bytes())?;
    std::fs::write(
        staging.join("oci-layout"),
        "{\"imageLayoutVersion\":\"1.0.0\"}\n".as_bytes(),
    )?;
    Ok(())
}

/// One image layer read from an image archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OciLayer {
    pub digest: String,
    pub size: u64,
    pub media_type: &'static str,
    pub diff_id: String,
}

/// The single image held by an image archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OciArchive {
    pub config: Vec<u8>,
    pub config_digest: String,
    pub layers: Vec<OciLayer>,
}

const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];

/// Compression detected from the first bytes of an archive or layer.
#[derive(Clone, Copy)]
enum OciCompression {
    None,
    Gzip,
    Zstd,
}

/// Detects gzip or zstd from a stream prefix.
fn oci_compression(head: &[u8]) -> OciCompression {
    if head.starts_with(&GZIP_MAGIC) {
        OciCompression::Gzip
    } else if head.starts_with(&ZSTD_MAGIC) {
        OciCompression::Zstd
    } else {
        OciCompression::None
    }
}
const OCI_LINK_HOPS: usize = 40;

fn oci_invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn oci_malformed(error: impl std::fmt::Display) -> io::Error {
    oci_invalid(format!("malformed image archive: {error}"))
}

/// Reads up to `buf.len()` bytes, stopping early only at end of stream.
fn read_prefix(reader: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
}

/// Opens one archive as a tar stream, unwrapping gzip by its magic bytes.
fn oci_open_archive(image: &Path) -> io::Result<tar::Archive<Box<dyn Read>>> {
    use std::io::Seek as _;
    let opened =
        |error: io::Error| io::Error::new(error.kind(), format!("{}: {error}", image.display()));
    let mut file = std::fs::File::open(image).map_err(opened)?;
    let mut magic = [0u8; 4];
    let got = read_prefix(&mut file, &mut magic).map_err(opened)?;
    file.seek(io::SeekFrom::Start(0)).map_err(opened)?;
    let file = io::BufReader::new(file);
    let reader: Box<dyn Read> = match oci_compression(&magic[..got]) {
        OciCompression::Gzip => Box::new(flate2::read::MultiGzDecoder::new(file)),
        OciCompression::Zstd => {
            return Err(oci_invalid(format!(
                "{}: unsupported image archive compression zstd; want .tar or .tar.gz",
                image.display()
            )));
        }
        OciCompression::None => Box::new(file),
    };
    Ok(tar::Archive::new(reader))
}

/// Joins a member path onto a base dir inside the archive, or None if it escapes.
fn oci_member_path(base: &str, path: &str) -> Option<String> {
    if path.starts_with('/') {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    for segment in base.split('/').chain(path.split('/')) {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

/// One link member inside an archive.
enum OciLink {
    Symbolic(String),
    Hard(String),
}

/// Follows archive links from one member name to the member that holds the bytes.
fn oci_resolve(
    links: &std::collections::HashMap<String, OciLink>,
    name: &str,
) -> io::Result<String> {
    let mut current = name.to_owned();
    for _ in 0..OCI_LINK_HOPS {
        let next = match links.get(&current) {
            None => return Ok(current),
            Some(OciLink::Symbolic(target)) => {
                let parent = current.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");
                oci_member_path(parent, target)
            }
            Some(OciLink::Hard(target)) => oci_member_path("", target),
        };
        current = next.ok_or_else(|| {
            oci_invalid(format!("image archive link '{name}' escapes the archive"))
        })?;
    }
    Err(oci_invalid(format!(
        "image archive link '{name}' does not resolve"
    )))
}

/// Reader that hashes, counts, and optionally copies every byte it yields.
struct OciTee<'a, R> {
    inner: R,
    hash: dx_digest::Sha256Writer,
    size: u64,
    copy: Option<&'a mut std::fs::File>,
}

impl<R: Read> Read for OciTee<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.hash.write_all(&buf[..read])?;
        if let Some(copy) = self.copy.as_mut() {
            copy.write_all(&buf[..read])?;
        }
        self.size += read as u64;
        Ok(read)
    }
}

/// Streams one layer, returning it with its blob digest and uncompressed diff_id.
fn oci_read_layer(
    reader: impl Read,
    name: &str,
    copy: Option<&mut std::fs::File>,
) -> io::Result<OciLayer> {
    let mut tee = OciTee {
        inner: reader,
        hash: dx_digest::Sha256Writer::new(),
        size: 0,
        copy,
    };
    let mut magic = [0u8; 4];
    let got = read_prefix(&mut tee, &mut magic)?;
    let compression = oci_compression(&magic[..got]);
    let head = io::Cursor::new(magic[..got].to_vec());
    let diff_id = match compression {
        OciCompression::Gzip => {
            dx_digest::sha256_reader_hex(flate2::read::MultiGzDecoder::new(head.chain(&mut tee)))
                .map_err(|error| {
                    oci_invalid(format!(
                        "image archive layer '{name}' is not valid gzip: {error}"
                    ))
                })?
        }
        OciCompression::Zstd => {
            return Err(oci_invalid(format!(
                "image archive layer '{name}' uses unsupported layer compression zstd; want uncompressed or gzip"
            )));
        }
        OciCompression::None => dx_digest::sha256_reader_hex(head.chain(&mut tee))?,
    };
    io::copy(&mut tee, &mut io::sink())?;
    Ok(OciLayer {
        digest: format!("sha256:{}", tee.hash.finish_hex()),
        size: tee.size,
        media_type: match compression {
            OciCompression::Gzip => "application/vnd.oci.image.layer.v1.tar+gzip",
            OciCompression::None | OciCompression::Zstd => "application/vnd.oci.image.layer.v1.tar",
        },
        diff_id: format!("sha256:{diff_id}"),
    })
}

const OCI_INDEX_DEPTH: usize = 8;

const OCI_INDEX_TYPES: [&str; 2] = [
    "application/vnd.oci.image.index.v1+json",
    "application/vnd.docker.distribution.manifest.list.v2+json",
];

const OCI_MANIFEST_TYPES: [&str; 2] = [
    "application/vnd.oci.image.manifest.v1+json",
    "application/vnd.docker.distribution.manifest.v2+json",
];

const OCI_LAYER_TYPES: [&str; 4] = [
    "application/vnd.oci.image.layer.v1.tar",
    "application/vnd.oci.image.layer.v1.tar+gzip",
    "application/vnd.docker.image.rootfs.diff.tar.gzip",
    "application/vnd.docker.image.rootfs.diff.tar",
];

type OciLinks = std::collections::HashMap<String, OciLink>;

/// Links and control files found by the first pass over an image archive.
struct OciScan {
    links: OciLinks,
    manifest_json: Option<Vec<u8>>,
    index_json: Option<Vec<u8>>,
}

/// Member names and expected digests of the single image an archive holds.
struct OciPlan {
    config: String,
    config_label: String,
    layers: Vec<String>,
    layer_labels: Vec<String>,
    config_digest: Option<String>,
    layer_digests: Option<Vec<String>>,
}

/// Collects links plus the manifest.json and index.json bodies in one pass.
fn oci_scan(image: &Path) -> io::Result<OciScan> {
    let mut scan = OciScan {
        links: OciLinks::new(),
        manifest_json: None,
        index_json: None,
    };
    for entry in oci_open_archive(image)?.entries().map_err(oci_malformed)? {
        let mut entry = entry.map_err(oci_malformed)?;
        let raw = entry
            .path()
            .map_err(oci_malformed)?
            .to_string_lossy()
            .into_owned();
        let Some(name) = oci_member_path("", &raw) else {
            continue;
        };
        let kind = entry.header().entry_type();
        if kind.is_symlink() || kind.is_hard_link() {
            let target = entry
                .link_name()
                .map_err(oci_malformed)?
                .map(|target| target.to_string_lossy().into_owned())
                .unwrap_or_default();
            let link = if kind.is_symlink() {
                OciLink::Symbolic(target)
            } else {
                OciLink::Hard(target)
            };
            scan.links.insert(name, link);
        } else if kind.is_file() && (name == "manifest.json" || name == "index.json") {
            let mut body = Vec::new();
            entry.read_to_end(&mut body).map_err(oci_malformed)?;
            if name == "manifest.json" {
                scan.manifest_json = Some(body);
            } else {
                scan.index_json = Some(body);
            }
        }
    }
    Ok(scan)
}

/// Reads the named regular-file members in one pass, keyed by the requested name.
fn oci_read_members(
    image: &Path,
    links: &OciLinks,
    names: &[String],
) -> io::Result<std::collections::HashMap<String, Vec<u8>>> {
    let mut wanted: std::collections::HashMap<String, Vec<&str>> = std::collections::HashMap::new();
    for name in names {
        wanted
            .entry(oci_resolve(links, name)?)
            .or_default()
            .push(name);
    }
    let mut found = std::collections::HashMap::new();
    for entry in oci_open_archive(image)?.entries().map_err(oci_malformed)? {
        let mut entry = entry.map_err(oci_malformed)?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let raw = entry
            .path()
            .map_err(oci_malformed)?
            .to_string_lossy()
            .into_owned();
        let Some(requested) = oci_member_path("", &raw).and_then(|name| wanted.remove(&name))
        else {
            continue;
        };
        let mut body = Vec::new();
        entry.read_to_end(&mut body).map_err(oci_malformed)?;
        for name in requested {
            found.insert(name.to_owned(), body.clone());
        }
    }
    Ok(found)
}

/// Builds the read plan from a docker save manifest.json.
fn oci_docker_plan(manifest: &[u8], links: &OciLinks) -> io::Result<OciPlan> {
    let parsed: serde_json::Value = serde_json::from_slice(manifest)
        .map_err(|error| oci_invalid(format!("malformed manifest.json: {error}")))?;
    let images = parsed
        .as_array()
        .ok_or_else(|| oci_invalid("malformed manifest.json: want an array"))?;
    if images.len() != 1 {
        return Err(oci_invalid(format!(
            "image archive holds {} images; want exactly one",
            images.len()
        )));
    }
    let config_name = images[0]
        .get("Config")
        .and_then(|value| value.as_str())
        .ok_or_else(|| oci_invalid("malformed manifest.json: missing Config"))?;
    let layer_names: Vec<&str> = images[0]
        .get("Layers")
        .and_then(|value| value.as_array())
        .map(|layers| layers.iter().filter_map(|layer| layer.as_str()).collect())
        .unwrap_or_default();
    if layer_names.is_empty() {
        return Err(oci_invalid("malformed manifest.json: missing Layers"));
    }
    let member = |path: &str| {
        oci_member_path("", path)
            .ok_or_else(|| {
                oci_invalid(format!(
                    "malformed manifest.json: path '{path}' escapes the archive"
                ))
            })
            .and_then(|name| oci_resolve(links, &name))
    };
    let config = member(config_name)?;
    let layers = layer_names
        .iter()
        .map(|layer| member(layer))
        .collect::<io::Result<Vec<String>>>()?;
    Ok(OciPlan {
        config,
        config_label: config_name.trim_start_matches("./").to_owned(),
        layers,
        layer_labels: layer_names.iter().map(|name| (*name).to_owned()).collect(),
        config_digest: None,
        layer_digests: None,
    })
}

/// Maps a sha256 digest to its blob member name.
fn oci_blob_member(digest: &str) -> io::Result<String> {
    match digest.strip_prefix("sha256:") {
        Some(hex)
            if hex.len() == 64
                && hex
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) =>
        {
            Ok(format!("blobs/sha256/{hex}"))
        }
        _ => Err(oci_invalid(format!(
            "image archive digest '{digest}' is not a sha256 digest"
        ))),
    }
}

/// Reads the blobs behind a set of descriptors and checks each against its digest.
fn oci_read_descriptors(
    image: &Path,
    links: &OciLinks,
    descriptors: &[serde_json::Value],
) -> io::Result<Vec<Vec<u8>>> {
    let digests: Vec<&str> = descriptors
        .iter()
        .map(|descriptor| descriptor["digest"].as_str().unwrap_or(""))
        .collect();
    let names = digests
        .iter()
        .map(|digest| oci_blob_member(digest))
        .collect::<io::Result<Vec<String>>>()?;
    let mut bodies = oci_read_members(image, links, &names)?;
    let mut read = Vec::with_capacity(names.len());
    for (position, (name, digest)) in names.iter().zip(&digests).enumerate() {
        // A digest listed twice shares one body: clone until its last use.
        let body = if names[position + 1..].contains(name) {
            bodies.get(name).cloned()
        } else {
            bodies.remove(name)
        }
        .ok_or_else(|| oci_invalid(format!("image archive is missing its blob '{digest}'")))?;
        if format!("sha256:{}", dx_digest::sha256_hex(&body)) != *digest {
            return Err(oci_invalid(format!(
                "image archive blob '{digest}' does not match its digest"
            )));
        }
        read.push(body);
    }
    Ok(read)
}

/// Drops descriptors repeating an earlier digest; the same manifest under two
/// `ref.name` tags or listed twice in an index is one image.
fn oci_dedupe_by_digest(descriptors: &mut Vec<serde_json::Value>) {
    let mut seen = std::collections::HashSet::new();
    descriptors.retain(|descriptor| match descriptor["digest"].as_str() {
        Some(digest) => seen.insert(digest.to_owned()),
        None => true,
    });
}

/// Builds the read plan from an OCI layout index.json.
fn oci_index_plan(image: &Path, index: &[u8], links: &OciLinks) -> io::Result<OciPlan> {
    let parsed: serde_json::Value = serde_json::from_slice(index)
        .map_err(|error| oci_invalid(format!("malformed index.json: {error}")))?;
    let mut level = parsed["manifests"]
        .as_array()
        .cloned()
        .ok_or_else(|| oci_invalid("malformed index.json: want a manifests array"))?;
    let mut images: Vec<serde_json::Value> = Vec::new();
    let mut depth = 0;
    loop {
        let mut nested: Vec<serde_json::Value> = Vec::new();
        for descriptor in level {
            if descriptor["annotations"]["vnd.docker.reference.type"] == "attestation-manifest" {
                continue;
            }
            let media_type = descriptor["mediaType"].as_str().unwrap_or("");
            if OCI_INDEX_TYPES.contains(&media_type) {
                nested.push(descriptor);
            } else if OCI_MANIFEST_TYPES.contains(&media_type) {
                images.push(descriptor);
            } else {
                return Err(oci_invalid(format!(
                    "unsupported manifest media type '{media_type}'"
                )));
            }
        }
        if nested.is_empty() {
            break;
        }
        depth += 1;
        if depth > OCI_INDEX_DEPTH {
            return Err(oci_invalid(format!(
                "image archive nests indexes deeper than {OCI_INDEX_DEPTH} levels"
            )));
        }
        oci_dedupe_by_digest(&mut nested);
        level = Vec::new();
        for body in oci_read_descriptors(image, links, &nested)? {
            let parsed: serde_json::Value = serde_json::from_slice(&body)
                .map_err(|error| oci_invalid(format!("malformed image index: {error}")))?;
            level.extend(
                parsed["manifests"]
                    .as_array()
                    .cloned()
                    .ok_or_else(|| oci_invalid("malformed image index: want a manifests array"))?,
            );
        }
    }
    oci_dedupe_by_digest(&mut images);
    if images.len() != 1 {
        return Err(oci_invalid(format!(
            "image archive holds {} images; want exactly one",
            images.len()
        )));
    }
    let body = oci_read_descriptors(image, links, &images)?.remove(0);
    let manifest: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|error| oci_invalid(format!("malformed image manifest: {error}")))?;
    let config_digest = manifest["config"]["digest"]
        .as_str()
        .ok_or_else(|| oci_invalid("malformed image manifest: missing config"))?
        .to_owned();
    let descriptors = manifest["layers"].as_array().cloned().unwrap_or_default();
    if descriptors.is_empty() {
        return Err(oci_invalid("malformed image manifest: missing layers"));
    }
    let mut layer_digests = Vec::with_capacity(descriptors.len());
    for descriptor in &descriptors {
        let media_type = descriptor["mediaType"].as_str().unwrap_or("");
        if !OCI_LAYER_TYPES.contains(&media_type) {
            return Err(oci_invalid(format!(
                "unsupported layer media type '{media_type}'"
            )));
        }
        let digest = descriptor["digest"]
            .as_str()
            .ok_or_else(|| oci_invalid("malformed image manifest: layer has no digest"))?;
        layer_digests.push(digest.to_owned());
    }
    let member = |digest: &str| oci_resolve(links, &oci_blob_member(digest)?);
    Ok(OciPlan {
        config: member(&config_digest)?,
        config_label: config_digest.clone(),
        layers: layer_digests
            .iter()
            .map(|digest| member(digest))
            .collect::<io::Result<Vec<String>>>()?,
        layer_labels: layer_digests.clone(),
        config_digest: Some(config_digest),
        layer_digests: Some(layer_digests),
    })
}

/// Reads the single image in an image archive, copying layers into `blobs` when set.
pub fn oci_read_archive(image: &Path, blobs: Option<&Path>) -> io::Result<OciArchive> {
    let scan = oci_scan(image)?;
    let plan = if let Some(manifest) = &scan.manifest_json {
        oci_docker_plan(manifest, &scan.links)?
    } else if let Some(index) = &scan.index_json {
        oci_index_plan(image, index, &scan.links)?
    } else {
        return Err(oci_invalid(
            "image archive has no manifest.json or index.json; want a docker save or OCI layout archive, not a rootfs layer",
        ));
    };
    oci_read_plan(image, &plan, blobs)
}

/// Streams the config and layers named by a plan in one pass and checks their digests.
fn oci_read_plan(image: &Path, plan: &OciPlan, blobs: Option<&Path>) -> io::Result<OciArchive> {
    let mut wanted: std::collections::HashMap<&str, Vec<usize>> = std::collections::HashMap::new();
    for (index, layer) in plan.layers.iter().enumerate() {
        wanted.entry(layer.as_str()).or_default().push(index);
    }
    let mut config: Option<Vec<u8>> = None;
    let mut layers: Vec<Option<OciLayer>> = vec![None; plan.layers.len()];
    for entry in oci_open_archive(image)?.entries().map_err(oci_malformed)? {
        let mut entry = entry.map_err(oci_malformed)?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let raw = entry
            .path()
            .map_err(oci_malformed)?
            .to_string_lossy()
            .into_owned();
        let Some(name) = oci_member_path("", &raw) else {
            continue;
        };
        let is_config = name == plan.config && config.is_none();
        let pending: Vec<usize> = wanted
            .get(name.as_str())
            .map(|indexes| {
                indexes
                    .iter()
                    .copied()
                    .filter(|&index| layers[index].is_none())
                    .collect()
            })
            .unwrap_or_default();
        if !is_config && pending.is_empty() {
            continue;
        }
        let read = if is_config {
            let mut body = Vec::new();
            entry.read_to_end(&mut body).map_err(oci_malformed)?;
            let read = if pending.is_empty() {
                None
            } else {
                Some(oci_copy_layer(io::Cursor::new(body.clone()), &name, blobs)?)
            };
            config = Some(body);
            read
        } else {
            Some(oci_copy_layer(&mut entry, &name, blobs)?)
        };
        if let Some(layer) = read {
            for index in pending {
                layers[index] = Some(layer.clone());
            }
        }
    }
    let config = config.ok_or_else(|| {
        oci_invalid(format!(
            "image archive is missing its config blob '{}'",
            plan.config_label
        ))
    })?;
    let config_digest = format!("sha256:{}", dx_digest::sha256_hex(&config));
    if plan
        .config_digest
        .as_ref()
        .is_some_and(|expected| *expected != config_digest)
    {
        return Err(oci_invalid(format!(
            "image archive blob '{}' does not match its digest",
            plan.config_label
        )));
    }
    let mut found = Vec::with_capacity(layers.len());
    for (index, (layer, label)) in layers.into_iter().zip(&plan.layer_labels).enumerate() {
        let layer = layer
            .ok_or_else(|| oci_invalid(format!("image archive is missing its layer '{label}'")))?;
        if plan
            .layer_digests
            .as_ref()
            .is_some_and(|expected| expected[index] != layer.digest)
        {
            return Err(oci_invalid(format!(
                "image archive blob '{label}' does not match its digest"
            )));
        }
        found.push(layer);
    }
    let parsed_config: serde_json::Value = serde_json::from_slice(&config)
        .map_err(|error| oci_invalid(format!("malformed image config: {error}")))?;
    let diff_ids: Vec<&str> = parsed_config["rootfs"]["diff_ids"]
        .as_array()
        .map(|ids| ids.iter().filter_map(|id| id.as_str()).collect())
        .unwrap_or_default();
    let computed: Vec<&str> = found.iter().map(|layer| layer.diff_id.as_str()).collect();
    if diff_ids != computed {
        return Err(oci_invalid(
            "image archive config rootfs.diff_ids do not match its layers",
        ));
    }
    Ok(OciArchive {
        config_digest,
        config,
        layers: found,
    })
}

/// Reads one layer, copying it into `blobs` under its digest when set.
fn oci_copy_layer(reader: impl Read, name: &str, blobs: Option<&Path>) -> io::Result<OciLayer> {
    let Some(blobs) = blobs else {
        return oci_read_layer(reader, name, None);
    };
    let temp = blobs.join(".layer.tmp");
    let mut file = std::fs::File::create(&temp)?;
    let layer = oci_read_layer(reader, name, Some(&mut file));
    drop(file);
    match layer {
        Ok(layer) => {
            std::fs::rename(
                &temp,
                blobs.join(layer.digest.trim_start_matches("sha256:")),
            )?;
            Ok(layer)
        }
        Err(error) => {
            let _ = std::fs::remove_file(&temp);
            Err(error)
        }
    }
}

/// Returns the config digest of the single image after the checks the layout enforces.
pub fn oci_declared_image_id(image: &Path, platform: &str) -> Result<String, String> {
    let (os, architecture) =
        oci_check_source(image, platform).map_err(|error| error.to_string())?;
    let archive = oci_read_archive(image, None).map_err(|error| error.to_string())?;
    oci_check_config(&archive, os, architecture).map_err(|error| error.to_string())?;
    Ok(archive.config_digest)
}

/// The caller's docker daemon endpoint, carried over to the scoped config.
#[derive(Debug, Default, PartialEq, Eq)]
struct DockerEndpoint {
    host: Option<String>,
    cert_path: Option<std::path::PathBuf>,
}

fn caller_docker_config_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("DOCKER_CONFIG")
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .map(|home| std::path::PathBuf::from(home).join(".docker"))
        })
}

/// Reads the caller's docker daemon endpoint.
fn docker_endpoint() -> Result<DockerEndpoint, String> {
    let cert_path = std::env::var_os("DOCKER_CERT_PATH")
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("DOCKER_TLS_VERIFY")
                .filter(|value| !value.is_empty())
                .and_then(|_| caller_docker_config_dir())
        });
    if let Some(host) = std::env::var("DOCKER_HOST")
        .ok()
        .filter(|value| !value.is_empty())
    {
        return Ok(DockerEndpoint {
            host: Some(host),
            cert_path,
        });
    }
    let output = std::process::Command::new("docker")
        .args(["context", "inspect", "--format", "{{json .}}"])
        .stderr(std::process::Stdio::null())
        .output();
    let value = output
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| serde_json::from_slice::<serde_json::Value>(&output.stdout).ok());
    let Some(value) = value else {
        return Ok(DockerEndpoint {
            host: None,
            cert_path,
        });
    };
    let has_tls = value
        .get("TLSMaterial")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|material| {
            material
                .values()
                .any(|files| files.as_array().is_some_and(|files| !files.is_empty()))
        });
    if has_tls {
        return Err(
            "the active docker context uses TLS, which a scoped docker config cannot carry; set DOCKER_HOST, DOCKER_TLS_VERIFY and DOCKER_CERT_PATH explicitly"
                .to_owned(),
        );
    }
    let host = value
        .pointer("/Endpoints/docker/Host")
        .and_then(serde_json::Value::as_str)
        .filter(|host| !host.is_empty())
        .map(str::to_owned);
    Ok(DockerEndpoint { host, cert_path })
}

/// Extra env names `docker` keeps beyond the publish base list.
const DOCKER_CHILD_ENV_EXTRA: &[&str] =
    &["DOCKER_TLS_VERIFY", "DOCKER_API_VERSION", "XDG_RUNTIME_DIR"];

/// Builds one docker command bound to the scoped config and the caller's daemon.
fn docker_command(config: &Path, endpoint: &DockerEndpoint) -> std::process::Command {
    let mut command = scrubbed_command(
        Path::new("docker"),
        &[PUBLISH_CHILD_ENV_KEEP, DOCKER_CHILD_ENV_EXTRA],
        &[],
    );
    command.env("DOCKER_CONFIG", config);
    if let Some(host) = &endpoint.host {
        command.env("DOCKER_HOST", host);
    }
    if let Some(cert_path) = &endpoint.cert_path {
        command.env("DOCKER_CERT_PATH", cert_path);
    }
    command
}

fn docker_image_id(
    docker: &dyn Fn() -> std::process::Command,
    reference: &str,
) -> Result<String, String> {
    let output = docker()
        .args(["image", "inspect", "--format", "{{.Id}}", reference])
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!("docker image inspect failed for {reference}"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Loads a docker-save archive and returns the image reference docker reports.
fn docker_load(docker: &dyn Fn() -> std::process::Command, image: &Path) -> Result<String, String> {
    let output = docker()
        .args(["load", "-q", "-i"])
        .arg(image)
        .stderr(std::process::Stdio::inherit())
        .output()
        .map_err(|_| "docker load failed".to_owned())?;
    if !output.status.success() {
        return Err("docker load failed".to_owned());
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| {
            line.strip_prefix("Loaded image ID: ")
                .or_else(|| line.strip_prefix("Loaded image: "))
                .map(|rest| rest.trim().to_owned())
        })
        .filter(|loaded| !loaded.is_empty())
        .ok_or_else(|| "docker load reported no loaded image".to_owned())
}

fn oci_live_push(
    image: &Path,
    registry: &str,
    repository: &str,
    tag: &str,
    platform: &str,
    user: &str,
    token: &str,
) -> i32 {
    let image_id = match oci_declared_image_id(image, platform) {
        Ok(id) => id,
        Err(error) => {
            eprintln!("oci_deploy: {error}");
            return 1;
        }
    };
    if which_on_path("docker").is_none() {
        eprintln!("oci_deploy: 'docker' CLI not found on PATH");
        return 1;
    }
    let reference = format!("{registry}/{repository}:{tag}");
    let endpoint = match docker_endpoint() {
        Ok(endpoint) => endpoint,
        Err(error) => {
            eprintln!("oci_deploy: {error}");
            return 1;
        }
    };
    let config_dir = match PrivateStageDir::new("oci-docker-config-") {
        Ok(dir) => dir,
        Err(error) => {
            eprintln!("oci_deploy: {error}");
            return 1;
        }
    };
    let config = serde_json::json!({ "auths": { registry: {} } });
    if let Err(error) = render_json(&config)
        .and_then(|text| write_secure_file(config_dir.path(), "config.json", &text))
    {
        eprintln!("oci_deploy: {error}");
        return 1;
    }
    let docker = || docker_command(config_dir.path(), &endpoint);
    let mut login = docker();
    login
        .arg("login")
        .arg(registry)
        .arg("-u")
        .arg(user)
        .arg("--password-stdin");
    let mut child = match login.stdin(std::process::Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(error) => {
            eprintln!("oci_deploy: {error}");
            return 1;
        }
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(token.as_bytes());
    }
    if !child.wait().map(|s| s.success()).unwrap_or(false) {
        eprintln!("oci_deploy: docker login failed");
        return 1;
    }
    let loaded = match docker_load(&docker, image) {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("oci_deploy: {error}; refusing to push");
            return 1;
        }
    };
    let tagged = docker().arg("tag").arg(&loaded).arg(&reference).status();
    if !tagged.map(|s| s.success()).unwrap_or(false) {
        eprintln!("oci_deploy: docker tag {loaded} {reference} failed");
        return 1;
    }
    match (
        docker_image_id(&docker, &loaded),
        docker_image_id(&docker, &reference),
    ) {
        (Ok(source), Ok(bound)) if source == bound => {
            println!("oci_deploy: bound {reference} to {bound} (config {image_id})");
        }
        (Ok(source), Ok(bound)) => {
            eprintln!(
                "oci_deploy: {reference} identifies {bound}, not the declared image {source}; refusing to push"
            );
            return 1;
        }
        (Err(error), _) | (_, Err(error)) => {
            eprintln!("oci_deploy: {error}; refusing to push");
            return 1;
        }
    }
    match docker().arg("push").arg(&reference).status() {
        Ok(status) if status.success() => {
            println!("oci_deploy: pushed {reference}");
            println!(
                "oci_deploy: signing-first: run cosign sign <digest> plus verify on the release trust root before updating scaffold refs"
            );
            0
        }
        Ok(status) => {
            eprintln!("oci_deploy: docker push failed with {status}");
            1
        }
        Err(error) => {
            eprintln!("oci_deploy: {error}");
            1
        }
    }
}

/// Owner approval gate for oci_deploy.
const OCI_GATE: OwnerGate = OwnerGate {
    tool: "oci_deploy",
    action: "push",
    credentials: &["OCI_REGISTRY_USER", "OCI_REGISTRY_TOKEN"],
    approved: "OCI_PUBLISH_APPROVED",
    live: "OCI_PUBLISH_LIVE",
};

/// Runs the OCI deploy launcher.
pub fn oci_main(
    image_rloc: &str,
    registry: &str,
    repository: &str,
    tag: &str,
    platform: &str,
    argv: &[String],
) -> i32 {
    let Some(run) = DeployRun::start(
        "oci_deploy",
        "the layout is exactly the file pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let outdir = run.outdir();
    let Some(image) = run.resolve(image_rloc) else {
        return 1;
    };
    if OCI_GATE.requested(env_or_empty) {
        if tag == "0.0.0" || tag == "0.0.0-dryrun" {
            eprintln!("oci_deploy: live push refuses placeholder tag; set a real tag");
            return 1;
        }
        let secrets = match OCI_GATE.authorize(env_or_empty) {
            Ok(secrets) => secrets,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        return oci_live_push(
            &image,
            registry,
            repository,
            tag,
            platform,
            secrets.get("OCI_REGISTRY_USER"),
            secrets.get("OCI_REGISTRY_TOKEN"),
        );
    }
    if std::env::var("OCI_PUBLISH_DRY_RUN").unwrap_or_default() == "1" {
        println!(
            "oci_deploy: dry run (OCI_PUBLISH_DRY_RUN=1); would push {registry}/{repository}:{tag}"
        );
        println!(
            "oci_deploy: signing-first: verify SBOM/provenance plus signing_demo before any live push, then cosign sign <digest>"
        );
    }
    match oci_build_layout(&image, outdir, registry, repository, tag, platform) {
        Ok(layout) => {
            let profile = deploy_profile();
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

/// Builds the argv for `octo push`.
pub fn octopus_push_args(package: &Path, spec: &OctopusDrop<'_>) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "push".into(),
        "--package".into(),
        package.as_os_str().to_owned(),
        "--server".into(),
        spec.server.into(),
    ];
    if !spec.space.is_empty() {
        args.push("--space".into());
        args.push(spec.space.into());
    }
    args
}

/// Builds the argv for `octo create-release`.
pub fn octopus_release_args(spec: &OctopusDrop<'_>) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "create-release".into(),
        "--project".into(),
        spec.project.into(),
        "--channel".into(),
        spec.channel.into(),
        "--version".into(),
        spec.version.into(),
    ];
    for env in spec.deploy_to {
        args.push("--deployTo".into());
        args.push(env.into());
    }
    args.push("--server".into());
    args.push(spec.server.into());
    if !spec.space.is_empty() {
        args.push("--space".into());
        args.push(spec.space.into());
    }
    args
}

/// Renders an octo argv as one display line.
fn octo_line(args: &[OsString]) -> String {
    let words: Vec<String> = args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    format!("octo {}", words.join(" "))
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
        octo_line(&octopus_push_args(Path::new(&base), spec)),
        octo_line(&octopus_release_args(spec)),
    );
    std::fs::write(drop.join("would-run.txt"), manifest.as_bytes())?;
    Ok(drop)
}

/// Owner approval gate for octopus_deploy.
const OCTOPUS_GATE: OwnerGate = OwnerGate {
    tool: "octopus_deploy",
    action: "push",
    credentials: &["OCTOPUS_API_KEY"],
    approved: "OCTOPUS_PUBLISH_APPROVED",
    live: "OCTOPUS_PUBLISH_LIVE",
};

/// Runs one octo step with the key in env only; true on success.
fn octo_run(octo: &Path, args: &[OsString], api_key: &str, step: &str) -> bool {
    let mut cmd = scrubbed_command(
        octo,
        &[PUBLISH_CHILD_ENV_KEEP],
        &[("OCTOPUS_CLI_API_KEY", api_key)],
    );
    cmd.args(args);
    match cmd.status() {
        Ok(status) if status.success() => true,
        Ok(status) => {
            eprintln!("octopus_deploy: octo {step} failed with {status}");
            false
        }
        Err(error) => {
            eprintln!("octopus_deploy: octo {step}: {error}");
            false
        }
    }
}

/// Pushes the package, then creates the release and queues each deployment.
pub fn octopus_live_publish(
    octo: &Path,
    package: &Path,
    spec: &OctopusDrop<'_>,
    api_key: &str,
) -> i32 {
    let name = package
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !octo_run(octo, &octopus_push_args(package, spec), api_key, "push") {
        return 1;
    }
    if !octo_run(octo, &octopus_release_args(spec), api_key, "create-release") {
        eprintln!(
            "octopus_deploy: package {name} is already pushed; a rerun will fail on push (FailIfExists)"
        );
        return 1;
    }
    println!("octopus_deploy: pushed package {name}");
    println!(
        "octopus_deploy: created release {} {} on channel {}",
        spec.project, spec.version, spec.channel
    );
    for env in spec.deploy_to {
        println!(
            "octopus_deploy: queued deploy of {} {} to {env}",
            spec.project, spec.version
        );
    }
    0
}

/// Runs the Octopus deploy launcher.
pub fn octopus_main(spec: &OctopusLaunch<'_>, argv: &[String]) -> i32 {
    let Some(run) = DeployRun::start(
        "octopus_deploy",
        "the drop is exactly the file pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let outdir = run.outdir();
    let Some(package) = run.resolve(spec.package_rloc) else {
        return 1;
    };
    let space = std::env::var("OCTOPUS_SPACE").unwrap_or_else(|_| spec.space_default.to_owned());
    let server = std::env::var("OCTOPUS_URL").unwrap_or_else(|_| spec.url_default.to_owned());
    let deploy_to: Vec<String> = spec
        .deploy_to_raw
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned())
        .collect();
    let drop_spec = OctopusDrop {
        deploy_name: spec.deploy_name,
        project: spec.project,
        channel: spec.channel,
        version: spec.version,
        deploy_to: &deploy_to,
        space: &space,
        server: &server,
    };
    if OCTOPUS_GATE.requested(env_or_empty) {
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
        let secrets = match OCTOPUS_GATE.authorize(env_or_empty) {
            Ok(secrets) => secrets,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        let Some(octo) = which_on_path("octo") else {
            eprintln!("octopus_deploy: 'octo' CLI not found on PATH");
            return 1;
        };
        return octopus_live_publish(&octo, &package, &drop_spec, secrets.get("OCTOPUS_API_KEY"));
    }
    match octopus_build_drop(&package, outdir, &drop_spec) {
        Ok(drop) => {
            let profile = deploy_profile();
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

/// Health result recorded for one promotion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromotionHealth {
    Passed,
    NotRequired,
    SkippedLocal,
}

impl PromotionHealth {
    /// Returns the value written to promotion.json.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::NotRequired => "not-required",
            Self::SkippedLocal => "skipped-local",
        }
    }

    /// Returns the health line written to would-run.txt.
    pub fn manifest_line(self) -> &'static str {
        match self {
            Self::Passed => "health: passed (PROMOTION_HEALTH_CMD exited 0)",
            Self::NotRequired => "health: not required; set PROMOTION_REQUIRE_HEALTH=1 plus PROMOTION_HEALTH_CMD to gate the promotion on a health check",
            Self::SkippedLocal => "health: skipped locally; set PROMOTION_REQUIRE_HEALTH=1 plus PROMOTION_HEALTH_CMD to gate the promotion on a health check",
        }
    }
}

/// Builds one promotion staging dir.
#[allow(clippy::too_many_arguments)]
pub fn promotion_build(
    artifact_src: &Path,
    outdir: &Path,
    deploy_name: &str,
    from_env: &str,
    to_env: &str,
    version: &str,
    secret_refs: &[String],
    health: PromotionHealth,
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
    let record = render_json(&serde_json::json!({
        "artifact": base,
        "artifact_sha256": digest,
        "deploy": deploy_name,
        "from_environment": from_env,
        "health": health.as_str(),
        "registry_auth": "env-only",
        "secret_refs": refs,
        "to_environment": to_env,
        "version": version,
    }))?;
    std::fs::write(promotion.join("promotion.json"), record.as_bytes())?;
    let refs_line = if refs.is_empty() {
        "secret-refs: none".to_owned()
    } else {
        format!("secret-refs: {} (names only, never values)", refs.join(","))
    };
    let manifest = format!(
        "# would-run manifest for promotion_deploy (publishes nothing; live gates need PROMOTION_LIVE=1 plus PROMOTION_APPROVED=1)\n{}\n{}\nrollback: set PROMOTION_ROLLBACK=1 plus PROMOTION_ROLLBACK_TO=<version> to record a rollback pointer (history lives in the promotion dir or registry)\nartifact-sha256: {digest}  {base}\n{refs_line}\nregistry-auth: env-only (PROMOTION_REGISTRY_USER/PROMOTION_REGISTRY_TOKEN), never from BUILD\n",
        promotion_promote_command(&base, from_env, to_env, version),
        health.manifest_line(),
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

/// Owner approval gate for promotion_deploy.
const PROMOTION_GATE: OwnerGate = OwnerGate {
    tool: "promotion_deploy",
    action: "promote",
    credentials: &[],
    approved: "PROMOTION_APPROVED",
    live: "PROMOTION_LIVE",
};

/// Runs the promotion deploy launcher.
pub fn promotion_main(
    artifact_rloc: &str,
    deploy_name: &str,
    from_env: &str,
    to_env: &str,
    version: &str,
    argv: &[String],
) -> i32 {
    let Some(run) = DeployRun::start(
        "promotion_deploy",
        "the promotion is exactly the file pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let outdir = run.outdir();
    let Some(artifact) = run.resolve(artifact_rloc) else {
        return 1;
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
            outdir,
            deploy_name,
            from_env,
            to_env,
            version,
            &secret_refs,
            PromotionHealth::SkippedLocal,
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
    if PROMOTION_GATE.requested(env_or_empty) {
        if version == "0.0.0" {
            eprintln!("promotion_deploy: live promote refuses version 0.0.0; set a real version");
            return 1;
        }
        if let Some(error) = PROMOTION_GATE.refusal(env_or_empty) {
            eprintln!("{error}");
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
        let mut health = PromotionHealth::NotRequired;
        if std::env::var("PROMOTION_REQUIRE_HEALTH").unwrap_or_default() == "1" {
            let health_cmd = std::env::var("PROMOTION_HEALTH_CMD").unwrap_or_default();
            if health_cmd.is_empty() {
                eprintln!(
                    "promotion_deploy: PROMOTION_REQUIRE_HEALTH=1 needs PROMOTION_HEALTH_CMD; refusing"
                );
                return 1;
            }
            match promotion_run_health_cmd(&health_cmd) {
                Ok(0) => health = PromotionHealth::Passed,
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
            outdir,
            deploy_name,
            from_env,
            to_env,
            version,
            &secret_refs,
            health,
        ) {
            Ok(promotion) => {
                eprintln!(
                    "promotion_deploy: live promotion {from_env} -> {to_env} version {version} is not implemented; gates passed and nothing was published"
                );
                eprintln!("  promotion: {}", promotion.display());
                return 1;
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
        outdir,
        deploy_name,
        from_env,
        to_env,
        version,
        &secret_refs,
        PromotionHealth::SkippedLocal,
    ) {
        Ok(promotion) => {
            let profile = deploy_profile();
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

/// Creates one deterministic npm pack.
#[allow(clippy::too_many_arguments)]
pub fn npm_create_pack(
    package: &str,
    tag: &str,
    registry: &str,
    access: &str,
    provenance: &str,
    tgz_out: &Path,
    feed_out: &Path,
    srcs: &[std::path::PathBuf],
) -> io::Result<String> {
    if let Some(error) = npm_access_error(package, access)
        .or_else(|| npm_provenance_error(access, registry, provenance))
    {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, error));
    }
    let mut members: std::collections::BTreeMap<String, (Vec<u8>, u32)> =
        std::collections::BTreeMap::new();
    for (src, rel) in npm_package_paths(srcs)? {
        let data = std::fs::read(&src)?;
        let member = format!("package/{rel}");
        let mode = {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                let bits = std::fs::metadata(&src)?.permissions().mode();
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
    let tar = tar_archive(
        members
            .iter()
            .map(|(member, (data, mode))| (member.as_str(), data.as_slice(), *mode)),
    )?;
    let gz = gzip_compress(&tar)?;
    if let Some(parent) = tgz_out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(tgz_out, &gz)?;
    let digest = dx_digest::sha256_file_hex(tgz_out)?;
    let names: Vec<String> = members.keys().cloned().collect();
    let tgz_base = basename(tgz_out)?;
    let feed = render_json(&serde_json::json!({
        "access": access,
        "files": names,
        "name": package,
        "provenance": provenance,
        "registry": registry,
        "sha256": digest,
        "tag": tag,
        "tarball": tgz_base,
    }))?;
    if let Some(parent) = feed_out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(feed_out, feed.as_bytes())?;
    Ok(digest)
}

/// Rejects an npm access mode the package name cannot publish with.
pub fn npm_access_error(package: &str, access: &str) -> Option<String> {
    let scoped = package.starts_with('@');
    if scoped
        && !package[1..].split_once('/').is_some_and(|(scope, name)| {
            !scope.is_empty() && !name.is_empty() && !name.contains('/')
        })
    {
        return Some(format!(
            "npm_deploy: invalid package '{package}': a scoped name is @scope/name"
        ));
    }
    if !scoped && package.contains('/') {
        return Some(format!(
            "npm_deploy: invalid package '{package}': only scoped names contain '/'"
        ));
    }
    match access {
        "public" => None,
        "restricted" if scoped => None,
        "restricted" => Some(format!(
            "npm_deploy: access 'restricted' needs a scoped package, got unscoped '{package}'"
        )),
        _ => Some(format!(
            "npm_deploy: invalid access '{access}': want 'public' or 'restricted'"
        )),
    }
}

fn npm_registry_host(registry: &str) -> &str {
    registry
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("")
}

/// Rejects a provenance policy the access mode or registry cannot honor.
pub fn npm_provenance_error(access: &str, registry: &str, provenance: &str) -> Option<String> {
    match provenance {
        "none" => None,
        "required" if access != "public" => Some(format!(
            "npm_deploy: provenance 'required' is not supported with access '{access}'; \
             use access 'public' or provenance 'none'"
        )),
        "required" if npm_registry_host(registry) != "registry.npmjs.org" => Some(format!(
            "npm_deploy: provenance 'required' is not supported by registry '{registry}'; \
             use https://registry.npmjs.org or provenance 'none'"
        )),
        "required" => None,
        _ => Some(format!(
            "npm_deploy: invalid provenance '{provenance}': want 'required' or 'none'"
        )),
    }
}

/// Rejects a required provenance policy when no signing identity is available.
pub fn npm_provenance_unavailable(provenance: &str) -> Option<String> {
    if provenance != "required" {
        return None;
    }
    let present = |key: &str| std::env::var_os(key).is_some_and(|value| !value.is_empty());
    if present("ACTIONS_ID_TOKEN_REQUEST_URL") || present("SIGSTORE_ID_TOKEN") {
        return None;
    }
    Some(
        "npm_deploy: provenance 'required' but no CI identity token is available \
         (ACTIONS_ID_TOKEN_REQUEST_URL or SIGSTORE_ID_TOKEN); publishing nothing"
            .to_owned(),
    )
}

/// Builds the npm publish arguments for one staged tarball.
pub fn npm_publish_args(
    tarball: &Path,
    tag: &str,
    access: &str,
    registry: &str,
    provenance: &str,
) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = vec![
        "publish".into(),
        tarball.into(),
        "--tag".into(),
        tag.into(),
        "--access".into(),
        access.into(),
    ];
    args.push(if provenance == "required" {
        "--provenance".into()
    } else {
        "--provenance=false".into()
    });
    args.push("--registry".into());
    args.push(registry.into());
    args
}

/// Lists one gzip tar member table.
pub fn npm_tar_members(tgz_path: &Path) -> io::Result<Vec<String>> {
    let file = std::fs::File::open(tgz_path)?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    let mut members = Vec::new();
    for entry in archive.entries()? {
        let entry = entry?;
        members.push(String::from_utf8_lossy(&entry.path_bytes()).into_owned());
    }
    Ok(members)
}

/// Renders one npmrc auth line.
pub fn npm_npmrc_line(host: &str, token: &str) -> String {
    format!("//{host}/:_authToken={token}\n")
}

/// Writes one owner-only npmrc in a private staged dir.
pub fn npm_write_npmrc(host: &str, token: &str) -> io::Result<std::path::PathBuf> {
    let dir = tempfile_stage_dir("npm-publish-")?;
    match write_secure_file(&dir, ".npmrc", &npm_npmrc_line(host, token)) {
        Ok(path) => Ok(path),
        Err(error) => {
            let _ = std::fs::remove_dir_all(&dir);
            Err(error)
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct NpmFeed {
    name: String,
    tag: String,
    registry: String,
    access: String,
    provenance: String,
    #[allow(dead_code)]
    tarball: String,
    sha256: String,
    files: Vec<String>,
}

fn parse_npm_feed(body: &str) -> io::Result<NpmFeed> {
    serde_json::from_str(body).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("feed JSON is invalid: {error}"),
        )
    })
}

/// Verifies one npm pack pair.
pub fn npm_verify_pack(
    tgz_path: &Path,
    feed_path: &Path,
) -> io::Result<(String, String, String, String, String)> {
    let body = std::fs::read_to_string(feed_path)?;
    let NpmFeed {
        name,
        tag,
        registry,
        access,
        provenance,
        sha256: expected,
        files,
        ..
    } = parse_npm_feed(&body)?;
    let actual = dx_digest::sha256_file_hex(tgz_path)?;
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
    if let Some(error) = npm_access_error(&name, &access)
        .or_else(|| npm_provenance_error(&access, &registry, &provenance))
    {
        return Err(io::Error::new(io::ErrorKind::InvalidData, error));
    }
    Ok((name, tag, registry, access, provenance))
}

/// Resolves the npm pack pair from explicit runfiles keys.
pub fn npm_find_pack_inputs(
    feed_rloc: &str,
    tarball_rloc: &str,
) -> io::Result<(std::path::PathBuf, std::path::PathBuf)> {
    let feed_path = resolve_runfile(feed_rloc)?;
    let tgz_path = resolve_runfile(tarball_rloc)?;
    Ok((feed_path, tgz_path))
}

/// Runs the npm pack tool.
pub fn npm_pack_main(argv: &[String]) -> i32 {
    if argv.len() < 9 {
        eprintln!(
            "usage: npm_pack <package> <tag> <registry> <access> <provenance> <tgz_out> <feed_out> <src...>"
        );
        return 1;
    }
    let package = argv[1].clone();
    let tag = argv[2].clone();
    let registry = argv[3].clone();
    let access = argv[4].clone();
    let provenance = argv[5].clone();
    let tgz_out = std::path::PathBuf::from(&argv[6]);
    let feed_out = std::path::PathBuf::from(&argv[7]);
    let srcs: Vec<std::path::PathBuf> = argv[8..].iter().map(std::path::PathBuf::from).collect();
    match npm_create_pack(
        &package,
        &tag,
        &registry,
        &access,
        &provenance,
        &tgz_out,
        &feed_out,
        &srcs,
    ) {
        Ok(_) => 0,
        Err(error) => {
            eprintln!("npm_pack: {error}");
            1
        }
    }
}

/// Extra env names `npm` keeps: CA and node settings plus the CI identity for provenance.
const NPM_CHILD_ENV_EXTRA: &[&str] = &[
    "CI",
    "NODE_EXTRA_CA_CERTS",
    "NODE_OPTIONS",
    "ComSpec",
    "GITHUB_ACTIONS",
    "GITHUB_WORKFLOW_REF",
    "GITHUB_REPOSITORY",
    "GITHUB_REPOSITORY_ID",
    "GITHUB_REPOSITORY_OWNER_ID",
    "GITHUB_SERVER_URL",
    "GITHUB_EVENT_NAME",
    "GITHUB_REF",
    "GITHUB_SHA",
    "GITHUB_RUN_ID",
    "GITHUB_RUN_ATTEMPT",
    "RUNNER_ENVIRONMENT",
    "ACTIONS_ID_TOKEN_REQUEST_URL",
    "ACTIONS_ID_TOKEN_REQUEST_TOKEN",
    "GITLAB_CI",
    "SIGSTORE_ID_TOKEN",
    "CI_",
    "GITLAB_",
];

/// Runs npm publish with a temporary owner-only npmrc, in a cleared env.
pub fn npm_run_publish(npm: &Path, host: &str, token: &str, args: &[std::ffi::OsString]) -> i32 {
    let npmrc = match npm_write_npmrc(host, token) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("npm_deploy: {error}");
            return 1;
        }
    };
    let status = scrubbed_command(npm, &[PUBLISH_CHILD_ENV_KEEP, NPM_CHILD_ENV_EXTRA], &[])
        .args(args)
        .arg("--userconfig")
        .arg(&npmrc)
        .status();
    if let Some(dir) = npmrc.parent() {
        let _ = std::fs::remove_dir_all(dir);
    }
    match status {
        Ok(status) if status.success() => 0,
        Ok(status) => status.code().unwrap_or(1),
        Err(error) => {
            eprintln!("npm_deploy: {error}");
            1
        }
    }
}

/// Owner approval gate for npm_deploy.
const NPM_GATE: OwnerGate = OwnerGate {
    tool: "npm_deploy",
    action: "publish",
    credentials: &["NPM_TOKEN"],
    approved: "NPM_PUBLISH_APPROVED",
    live: "NPM_PUBLISH_LIVE",
};

/// Runs the npm deploy launcher.
pub fn npm_deploy_main(argv: &[String], feed_rloc: &str, tarball_rloc: &str) -> i32 {
    if std::env::var_os("NPM_PUBLISH_DRY_RUN").is_some() {
        eprintln!(
            "npm_deploy: NPM_PUBLISH_DRY_RUN is not supported; publishing needs NPM_PUBLISH_LIVE=1"
        );
        return 1;
    }
    let Some(run) = DeployRun::start(
        "npm_deploy",
        "the package is exactly the files pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let outdir = run.outdir();
    let (feed_path, tgz_path) = match npm_find_pack_inputs(feed_rloc, tarball_rloc) {
        Ok(pair) => pair,
        Err(error) => {
            eprintln!("npm_deploy: {error}");
            return 1;
        }
    };
    let (name, tag, registry, access, provenance) = match npm_verify_pack(&tgz_path, &feed_path) {
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
        if dx_atomic_fs::copy_atomic(src, &outdir.join(base)).is_err() {
            eprintln!("npm_deploy: cannot copy {}", src.display());
            return 1;
        }
        if dx_atomic_fs::copy_atomic(src, &feed_dir.join(base)).is_err() {
            eprintln!("npm_deploy: cannot copy {}", src.display());
            return 1;
        }
    }
    let profile = deploy_profile();
    println!("npm_deploy: released {name} (tag {tag}, access {access}, profile: {profile})");
    println!("  registry: {registry}");
    println!("  provenance: {provenance}");
    println!("  tarball: {}", outdir.join(&tgz_base).display());
    println!("  feed: {}", feed_dir.display());
    if NPM_GATE.requested(env_or_empty) {
        let secrets = match NPM_GATE.authorize(env_or_empty) {
            Ok(secrets) => secrets,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        if let Some(error) = npm_provenance_unavailable(&provenance) {
            eprintln!("{error}");
            return 1;
        }
        let staged_tgz = outdir.join(&tgz_base);
        let Some(npm) = which_on_path("npm") else {
            eprintln!("npm_deploy: 'npm' CLI not found on PATH");
            return 1;
        };
        let host = npm_registry_host(&registry);
        if host.is_empty() {
            eprintln!("npm_deploy: invalid registry '{registry}'");
            return 1;
        }
        return npm_run_publish(
            &npm,
            host,
            secrets.get("NPM_TOKEN"),
            &npm_publish_args(&staged_tgz, &tag, &access, &registry, &provenance),
        );
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    #[test]
    fn octal_fields_match_python_tarfile() {
        let header = ustar_header("hello", 12, 0o755).expect("header");
        let bytes = header.as_bytes();
        assert_eq!(&bytes[100..108], b"0000755\0");
        assert_eq!(&bytes[108..116], b"0000000\0");
        assert_eq!(&bytes[116..124], b"0000000\0");
        assert_eq!(&bytes[124..136], b"00000000014\0");
        assert_eq!(&bytes[136..148], b"00000000000\0");
        assert_eq!(&bytes[148..156], b"006771\0 ");
    }

    #[test]
    fn header_bytes_match_python_ustar() {
        let header = ustar_header("hello", 12, 0o755).expect("header");
        let bytes = header.as_bytes();
        assert_eq!(&bytes[0..6], b"hello\0");
        assert!(bytes[6..100].iter().all(|byte| *byte == 0));
        assert_eq!(&bytes[100..108], b"0000755\0");
        assert_eq!(&bytes[108..116], b"0000000\0");
        assert_eq!(&bytes[116..124], b"0000000\0");
        assert_eq!(&bytes[124..136], b"00000000014\0");
        assert_eq!(&bytes[136..148], b"00000000000\0");
        assert_eq!(&bytes[148..156], b"006771\0 ");
        assert_eq!(bytes[156], b'0');
        assert!(bytes[157..257].iter().all(|byte| *byte == 0));
        assert_eq!(&bytes[257..265], b"ustar\x0000");
        assert!(bytes[265..512].iter().all(|byte| *byte == 0));
        let plain = ustar_header("hello", 12, 0o644).expect("plain header");
        let plain_bytes = plain.as_bytes();
        assert_eq!(&plain_bytes[100..108], b"0000644\0");
        assert_ne!(&plain_bytes[148..156], &bytes[148..156]);
    }

    #[test]
    fn header_rejects_bad_names() {
        assert!(ustar_header("", 0, 0o644).is_err());
        assert!(ustar_header(&"n".repeat(101), 0, 0o644).is_err());
        assert!(ustar_header(&"n".repeat(300), 0, 0o644).is_err());
        assert!(ustar_header("a\0b", 0, 0o644).is_err());
        assert!(ustar_header("../escape", 0, 0o644).is_err());
        assert!(ustar_header("/absolute", 0, 0o644).is_err());
    }

    #[test]
    fn tar_image_pads_to_recordsize() {
        let tiny = tar_archive([("f", b"hi".as_slice(), 0o644)].into_iter()).expect("tiny");
        assert_eq!(tiny.len(), RECORDSIZE);
        assert_eq!(&tiny[0..1], b"f");
        assert_eq!(&tiny[512..514], b"hi");
        assert!(tiny[514..].iter().all(|byte| *byte == 0));
        let big_data = vec![b'x'; 10240];
        let big = tar_archive([("f", big_data.as_slice(), 0o644)].into_iter()).expect("big");
        assert_eq!(big.len(), 2 * RECORDSIZE);
    }

    #[test]
    fn gzip_header_matches_python_gzip() {
        let tar =
            tar_archive([("hello", b"hello world\n".as_slice(), 0o755)].into_iter()).expect("tar");
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
    fn archive_file_marks_the_member_executable_and_follows_symlinks() {
        let scratch = scratch_dir();
        let executable = scratch.path().join("run.sh");
        std::fs::write(&executable, b"#!/bin/sh\necho hi\n").expect("write exe");
        let plain = scratch.path().join("data.txt");
        std::fs::write(&plain, b"plain\n").expect("write plain");
        let exe_out = scratch.path().join("exe.tar.gz");
        archive_file(&executable, &exe_out).expect("archive exe");
        let plain_out = scratch.path().join("plain.tar.gz");
        archive_file(&plain, &plain_out).expect("archive plain");
        for path in [&exe_out, &plain_out] {
            let gz = std::fs::read(path).expect("read gz");
            let mut decoder = flate2::read::GzDecoder::new(&gz[..]);
            let mut tar = Vec::new();
            use std::io::Read as _;
            decoder.read_to_end(&mut tar).expect("decode");
            let mode = String::from_utf8_lossy(&tar[100..107]).into_owned();
            assert_eq!(mode, "0000755");
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
    fn hash_file_writes_sha256sum_line() {
        let scratch = scratch_dir();
        let src = scratch.path().join("payload.bin");
        let payload = b"archive payload\n";
        std::fs::write(&src, payload).expect("write");
        let dst = scratch.path().join("payload.sha256");
        hash_file(&src, &dst).expect("hash");
        let text = std::fs::read_to_string(&dst).expect("read line");
        assert_eq!(
            text,
            format!("{}  payload.bin\n", dx_digest::sha256_hex(payload))
        );
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
        let app = scratch.path().join("app_name.txt");
        std::fs::write(&app, b"release_demo_app\n").expect("write app");
        let tarball = scratch.path().join("release_demo.tar.gz");
        std::fs::write(&tarball, b"archive-tarball-bytes-v1").expect("write tarball");
        let digest = dx_digest::sha256_file_hex(&tarball).expect("digest");
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
        assert_eq!(app_name, "release_demo_app");
        assert!(tarball_dest.is_file());
        assert!(checksum_dest.is_file());
        assert_eq!(
            dx_digest::sha256_file_hex(&tarball_dest).expect("hex"),
            dx_digest::sha256_file_hex(&tarball).expect("hex")
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
    fn archive_stage_rejects_an_empty_app_name() {
        let scratch = scratch_dir();
        let app = scratch.path().join("app_name.txt");
        std::fs::write(&app, b" \n").expect("write app");
        let tarball = scratch.path().join("release_demo.tar.gz");
        std::fs::write(&tarball, b"bytes").expect("write tarball");
        let checksum = scratch.path().join("release_demo.tar.gz.sha256");
        std::fs::write(&checksum, b"00  release_demo.tar.gz\n").expect("write checksum");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let err = archive_stage_release(&app, &tarball, &checksum, &outdir)
            .expect_err("an empty app name must fail");
        assert!(err.to_string().contains("names no app"), "{err}");
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
        std::fs::write(&wheel, b"w").expect("wheel");
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
    fn github_staging_rejects_duplicate_basename() {
        let scratch = scratch_dir();
        let first = scratch.path().join("a").join("app.tar.gz");
        let second = scratch.path().join("b").join("app.tar.gz");
        for (path, bytes) in [(&first, b"linux-bytes"), (&second, b"macos-bytes")] {
            std::fs::create_dir_all(path.parent().expect("parent")).expect("asset dir");
            std::fs::write(path, bytes).expect("write asset");
        }
        let outdir = scratch.path().join("out");
        let error = github_build_staging(
            &[second.clone(), first.clone()],
            &outdir,
            "github_dup",
            "v0.0.0-dryrun",
        )
        .expect_err("duplicate basename must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        let message = error.to_string();
        assert!(message.contains(&first.display().to_string()), "{message}");
        assert!(message.contains(&second.display().to_string()), "{message}");
        assert!(
            message.contains("share the asset name 'app.tar.gz'"),
            "{message}"
        );
        assert!(!outdir.join("github_dup-release").exists());
    }

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

    #[test]
    fn oci_layout_builds_index() {
        let scratch = scratch_dir();
        let (image, _) = oci_save(
            scratch.path(),
            "oci_demo.tar",
            &[oci_test_layer(false)],
            "linux/amd64",
        );
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let layout = oci_build_layout(
            &image,
            &outdir,
            "registry.example.invalid",
            "demo/app",
            "1.2.3",
            "linux/amd64",
        )
        .expect("layout");
        assert!(layout.join("oci-layout").is_file());
        assert!(layout.join("blobs").join("sha256").is_dir());
        assert!(!outdir.join("app-oci-layout.tmp").exists());
        let index = std::fs::read_to_string(layout.join("index.json")).expect("index");
        assert!(!index.contains('\n'), "index is compact");
        let parsed: serde_json::Value = serde_json::from_str(&index).expect("index parses");
        assert_eq!(parsed["schemaVersion"], 2);
        assert_eq!(
            parsed["mediaType"],
            "application/vnd.oci.image.index.v1+json"
        );
        assert_eq!(
            parsed["manifests"][0]["annotations"]["org.opencontainers.image.ref.name"],
            "registry.example.invalid/demo/app:1.2.3"
        );
        let digest = parsed["manifests"][0]["digest"].as_str().expect("digest");
        let manifest = layout.join("blobs").join("sha256").join(&digest[7..]);
        let manifest_bytes = std::fs::read(&manifest).expect("manifest");
        assert!(!manifest_bytes.contains(&b'\n'), "manifest is compact");
        assert_eq!(parsed["manifests"][0]["size"], manifest_bytes.len());
        assert_eq!(
            digest,
            format!("sha256:{}", dx_digest::sha256_hex(&manifest_bytes))
        );
        assert_eq!(
            digest,
            format!(
                "sha256:{}",
                dx_digest::sha256_file_hex(&manifest).expect("hex")
            )
        );
        let again = oci_build_layout(
            &image,
            &outdir,
            "registry.example.invalid",
            "demo/app",
            "1.2.3",
            "linux/amd64",
        )
        .expect("rebuild layout");
        assert_eq!(again, layout);
        assert_eq!(
            std::fs::read_to_string(layout.join("index.json")).expect("index"),
            index
        );
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
        let manifest = std::fs::read_to_string(drop.join("would-run.txt")).expect("manifest");
        let spec = OctopusDrop {
            deploy_name: "octopus_demo",
            project: "octopus_demo",
            channel: "Default",
            version: "0.0.0",
            deploy_to: &["Production".to_owned()],
            space: "",
            server: "https://octopus.example.invalid",
        };
        let lines: Vec<&str> = manifest.lines().collect();
        assert_eq!(
            lines[1],
            "octo push --package release_demo.tar.gz --server https://octopus.example.invalid"
        );
        assert_eq!(
            lines[2],
            "octo create-release --project octopus_demo --channel Default --version 0.0.0 --deployTo Production --server https://octopus.example.invalid"
        );
        assert_eq!(
            lines[1],
            octo_line(&octopus_push_args(Path::new("release_demo.tar.gz"), &spec))
        );
        assert_eq!(lines[2], octo_line(&octopus_release_args(&spec)));
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
            PromotionHealth::SkippedLocal,
        )
        .expect("promotion");
        let record = std::fs::read_to_string(promotion.join("promotion.json")).expect("record");
        let artifact_digest = dx_digest::sha256_file_hex(&artifact).expect("hex");
        assert!(record.ends_with('\n'));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&record).expect("record parses"),
            serde_json::json!({
                "artifact": basename(&artifact).expect("base"),
                "artifact_sha256": artifact_digest,
                "deploy": "promotion_demo",
                "from_environment": "staging",
                "health": "skipped-local",
                "registry_auth": "env-only",
                "secret_refs": [],
                "to_environment": "production",
                "version": "1.2.3",
            })
        );
        assert!(promotion.join("would-run.txt").is_file());
        let passed_out = scratch.path().join("passed");
        std::fs::create_dir_all(&passed_out).expect("passed outdir");
        let passed = promotion_build(
            &artifact,
            &passed_out,
            "promotion_demo",
            "staging",
            "production",
            "1.2.3",
            &[],
            PromotionHealth::Passed,
        )
        .expect("passed promotion");
        let passed_record: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(passed.join("promotion.json")).expect("passed record"),
        )
        .expect("passed record parses");
        assert_eq!(passed_record["health"], "passed");
        let manifest = std::fs::read_to_string(passed.join("would-run.txt")).expect("manifest");
        assert!(manifest.contains("health: passed"));
        assert!(!manifest.contains("local default"));
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
            "public",
            "required",
            &tgz,
            &feed,
            std::slice::from_ref(&src),
        )
        .expect("pack");
        assert_eq!(digest, dx_digest::sha256_file_hex(&tgz).expect("hex"));
        let (name, tag, registry, _, _) = npm_verify_pack(&tgz, &feed).expect("verify");
        assert_eq!(name, "npm-demo");
        assert_eq!(tag, "latest");
        assert_eq!(registry, "https://registry.npmjs.org");
        let members = npm_tar_members(&tgz).expect("members");
        assert_eq!(members, vec!["package/package.json".to_owned()]);
        assert_eq!(
            npm_npmrc_line("registry.npmjs.org", "tok"),
            "//registry.npmjs.org/:_authToken=tok\n"
        );
        let body = std::fs::read_to_string(&feed).expect("feed");
        assert!(body.ends_with('\n'));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&body).expect("feed parses"),
            serde_json::json!({
                "access": "public",
                "files": ["package/package.json"],
                "name": "npm-demo",
                "provenance": "required",
                "registry": "https://registry.npmjs.org",
                "sha256": digest,
                "tag": "latest",
                "tarball": "npm_demo.tgz",
            })
        );
    }

    #[test]
    fn npm_tar_members_rejects_a_bad_header_checksum() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(&src, b"{\"name\": \"npm-demo\", \"version\": \"0.0.0\"}\n")
            .expect("write src");
        let good = scratch.path().join("good.tgz");
        npm_create_pack(
            "npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "public",
            "required",
            &good,
            &scratch.path().join("good.feed.json"),
            std::slice::from_ref(&src),
        )
        .expect("pack");
        assert_eq!(npm_tar_members(&good).expect("members").len(), 1);
        let mut tar = Vec::new();
        use std::io::Read as _;
        flate2::read::GzDecoder::new(std::fs::File::open(&good).expect("open tgz"))
            .read_to_end(&mut tar)
            .expect("decode");
        tar[100] ^= 1;
        let bad = scratch.path().join("bad.tgz");
        std::fs::write(&bad, gzip_compress(&tar).expect("gz")).expect("write bad");
        assert!(npm_tar_members(&bad).is_err());
    }

    fn nested_npm_package(root: &Path) -> Vec<std::path::PathBuf> {
        vec![
            write_crate_file(
                root,
                "package.json",
                "{\"name\": \"npm-nested\", \"version\": \"1.0.0\", \"main\": \"dist/index.js\", \"types\": \"dist/index.d.ts\", \"exports\": {\".\": \"./dist/index.js\", \"./util\": \"./dist/util/index.js\"}, \"bin\": {\"npm-nested\": \"bin/cli.js\"}}\n",
            ),
            write_crate_file(
                root,
                "dist/index.js",
                "const util = require('./util');\nmodule.exports = { value: 41 + util.one };\n",
            ),
            write_crate_file(root, "dist/index.d.ts", "export const value: number;\n"),
            write_crate_file(root, "dist/util/index.js", "module.exports = { one: 1 };\n"),
            write_crate_file(
                root,
                "bin/cli.js",
                "#!/usr/bin/env node\nconsole.log(require('../dist').value);\n",
            ),
        ]
    }

    fn pack_nested_npm(scratch: &Path) -> std::path::PathBuf {
        let srcs = nested_npm_package(&scratch.join("in/pkg"));
        let tgz = scratch.join("nested.tgz");
        npm_create_pack(
            "npm-nested",
            "latest",
            "https://registry.npmjs.org",
            "public",
            "required",
            &tgz,
            &scratch.join("nested.feed.json"),
            &srcs,
        )
        .expect("pack");
        tgz
    }

    #[test]
    fn npm_pack_preserves_package_paths() {
        let scratch = scratch_dir();
        let tgz = pack_nested_npm(scratch.path());
        assert_eq!(
            npm_tar_members(&tgz).expect("members"),
            vec![
                "package/bin/cli.js".to_owned(),
                "package/dist/index.d.ts".to_owned(),
                "package/dist/index.js".to_owned(),
                "package/dist/util/index.js".to_owned(),
                "package/package.json".to_owned(),
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn npm_pack_keeps_executable_modes() {
        use std::os::unix::fs::PermissionsExt as _;
        let scratch = scratch_dir();
        let srcs = nested_npm_package(&scratch.path().join("in/pkg"));
        let cli = &srcs[4];
        std::fs::set_permissions(cli, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let tgz = scratch.path().join("modes.tgz");
        npm_create_pack(
            "npm-nested",
            "latest",
            "https://registry.npmjs.org",
            "public",
            "required",
            &tgz,
            &scratch.path().join("modes.feed.json"),
            &srcs,
        )
        .expect("pack");
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(
            std::fs::File::open(&tgz).expect("open"),
        ));
        let mut modes = std::collections::BTreeMap::new();
        for entry in archive.entries().expect("entries") {
            let entry = entry.expect("entry");
            let name = String::from_utf8_lossy(&entry.path_bytes()).into_owned();
            modes.insert(name, entry.header().mode().expect("mode"));
        }
        assert_eq!(modes["package/bin/cli.js"], 0o755);
        assert_eq!(modes["package/dist/index.js"], 0o644);
    }

    #[test]
    fn npm_pack_allows_equal_basenames_in_different_directories() {
        let scratch = scratch_dir();
        let tgz = pack_nested_npm(scratch.path());
        let members = npm_tar_members(&tgz).expect("members");
        let indexes = members.iter().filter(|m| m.ends_with("/index.js")).count();
        assert_eq!(indexes, 2);
    }

    #[test]
    fn npm_package_paths_reject_collisions_and_escapes() {
        let scratch = scratch_dir();
        let root = scratch.path().join("in");
        let manifest = write_crate_file(&root, "package.json", "{}");
        let outside = write_crate_file(scratch.path(), "other/index.js", "");
        let error = npm_package_paths(&[manifest.clone(), outside]).expect_err("outside");
        assert!(
            error.to_string().contains("outside the package root"),
            "{error}"
        );
        write_crate_file(&root, "index.js", "");
        let error = npm_package_paths(&[manifest.clone(), root.join("src/../index.js")])
            .expect_err("dotdot");
        assert!(error.to_string().contains("unsafe package path"), "{error}");
        let error = npm_package_paths(&[
            manifest.clone(),
            root.join("index.js"),
            root.join("index.js"),
        ])
        .expect_err("dup");
        assert!(error.to_string().contains("path collision"), "{error}");
        let error = npm_package_paths(&[manifest.clone(), root.join("src"), root.join("src/a.js")])
            .expect_err("file+dir");
        assert!(
            error.to_string().contains("both a file and a directory"),
            "{error}"
        );
        let error = npm_package_paths(&[root.join("index.js")]).expect_err("no manifest");
        assert!(error.to_string().contains("no package.json"), "{error}");
        let error = npm_package_paths(&[
            write_crate_file(&root, "a/package.json", "{}"),
            write_crate_file(&root, "b/package.json", "{}"),
        ])
        .expect_err("ambiguous");
        assert!(
            error.to_string().contains("ambiguous package root"),
            "{error}"
        );
        assert!(npm_package_paths(&[]).is_err());
    }

    #[test]
    fn npm_pack_installs_into_a_local_consumer() {
        let ok = |program: &str| {
            std::process::Command::new(program)
                .arg("--version")
                .output()
                .map(|out| out.status.success())
                .unwrap_or(false)
        };
        if !ok("node") || !ok("npm") {
            return;
        }
        let scratch = scratch_dir();
        let tgz = pack_nested_npm(scratch.path());
        let consumer = scratch.path().join("consumer");
        write_crate_file(
            &consumer,
            "package.json",
            "{\"name\": \"consumer\", \"version\": \"0.0.0\", \"private\": true}\n",
        );
        let install = std::process::Command::new("npm")
            .args(["install", "--offline", "--no-audit", "--no-fund"])
            .arg(&tgz)
            .env("npm_config_cache", scratch.path().join("npm-cache"))
            .current_dir(&consumer)
            .output()
            .expect("npm install");
        assert!(
            install.status.success(),
            "{}",
            String::from_utf8_lossy(&install.stderr)
        );
        let run = std::process::Command::new("node")
            .args([
                "-e",
                "console.log(require('npm-nested').value + require('npm-nested/util').one)",
            ])
            .current_dir(&consumer)
            .output()
            .expect("node");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "43");
        assert!(consumer
            .join("node_modules/npm-nested/dist/index.d.ts")
            .is_file());
        assert!(consumer.join("node_modules/.bin/npm-nested").exists());
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
    fn render_json_escapes_control_and_markup() {
        let value = serde_json::json!({"k": "a\"b\\c\nd\u{1}<&>"});
        let text = render_json(&value).expect("renders");
        assert_eq!(text, "{\n  \"k\": \"a\\\"b\\\\c\\nd\\u0001<&>\"\n}\n");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&text).expect("parses"),
            value
        );
        assert_eq!(
            quick_xml::escape::escape("<a href=\"x\">&'</a>"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&apos;&lt;/a&gt;"
        );
    }

    #[test]
    fn npm_feed_parser_reads_escapes_brackets_and_unicode() {
        let body = r#"{ "tarball":"a.tgz", "tag":"t", "sha256":"d", "registry":"r", "access":"public", "provenance":"none",
            "name": "a\"b\\c\nd", "extra": [1, {"x": "]"}],
            "files": ["package/[id].js", "package/]x[.js", "y\\z", "w\"q", "\u00e6\u0041", "æ"] }"#;
        let feed = parse_npm_feed(body).expect("feed");
        assert_eq!(feed.name, "a\"b\\c\nd");
        assert_eq!(
            feed.files,
            [
                "package/[id].js",
                "package/]x[.js",
                "y\\z",
                "w\"q",
                "\u{e6}A",
                "\u{e6}"
            ]
        );
    }

    #[test]
    fn npm_feed_parser_rejects_bad_documents() {
        let good = r#"{"access": "public", "files": [], "name": "n", "provenance": "none", "registry": "r", "sha256": "d", "tag": "t", "tarball": "a.tgz"}"#;
        parse_npm_feed(good).expect("good");
        let bad = [
            "",
            "[]",
            r#"{"name": "#,
            r#"{"files": ["a"#,
            r#"{"files": [], "name": "n", "registry": "r", "sha256": "d", "tag": "t"}"#,
            r#"{"files": [], "name": "n", "registry": "r", "sha256": "d", "tag": "t", "tarball": "a"}"#,
            r#"{"files": 7, "name": "n", "registry": "r", "sha256": "d", "tag": "t", "tarball": "a"}"#,
            r#"{"files": [1], "name": "n", "registry": "r", "sha256": "d", "tag": "t", "tarball": "a"}"#,
            r#"{"files": [], "name": 7, "registry": "r", "sha256": "d", "tag": "t", "tarball": "a"}"#,
            r#"{"files": [], "name": "n", "name": "m", "registry": "r", "sha256": "d", "tag": "t", "tarball": "a"}"#,
            r#"{"files": [], "files": ["x"], "name": "n", "registry": "r", "sha256": "d", "tag": "t", "tarball": "a"}"#,
        ];
        for body in bad {
            let error = parse_npm_feed(body).expect_err(body);
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{body}");
        }
    }

    #[test]
    fn octopus_args_builders_cover_optional_segments() {
        let envs = ["Production".to_owned(), "Staging".to_owned()];
        let spec = |deploy_to: &'static [String], space: &'static str| OctopusDrop {
            deploy_name: "demo",
            project: "demo",
            channel: "Default",
            version: "1.0.0",
            deploy_to,
            space,
            server: "https://octo.invalid",
        };
        let envs: &'static [String] = Box::leak(Box::new(envs));
        let lines = |args: Vec<OsString>| octo_line(&args);
        let pkg = Path::new("demo-1.0.0.tar.gz");
        assert_eq!(
            lines(octopus_push_args(pkg, &spec(&[], ""))),
            "octo push --package demo-1.0.0.tar.gz --server https://octo.invalid"
        );
        assert_eq!(
            lines(octopus_push_args(pkg, &spec(&[], "Spaces"))),
            "octo push --package demo-1.0.0.tar.gz --server https://octo.invalid --space Spaces"
        );
        assert_eq!(
            lines(octopus_release_args(&spec(&[], ""))),
            "octo create-release --project demo --channel Default --version 1.0.0 --server https://octo.invalid"
        );
        let release = lines(octopus_release_args(&spec(envs, "Spaces")));
        assert_eq!(
            release,
            "octo create-release --project demo --channel Default --version 1.0.0 --deployTo Production --deployTo Staging --server https://octo.invalid --space Spaces"
        );
        let push = lines(octopus_push_args(pkg, &spec(envs, "Spaces")));
        for line in [&push, &release] {
            assert!(!line.contains("--deploy-to") && !line.contains("--apiKey"));
            assert!(!line.to_lowercase().contains("key"));
        }
        assert!(!release.contains("--package"));
    }

    #[test]
    fn command_formatters_cover_optional_segments() {
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
    fn stage_helpers_create_and_report_paths() {
        let dir = tempfile_stage_dir("dx-deploy-stage-").expect("stage dir");
        assert!(dir.is_dir());
        assert!(dir.to_string_lossy().contains("dx-deploy-stage-"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&dir).expect("stat").permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "stage dir must stay owner-only");
        }
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
        assert_eq!(digest, dx_digest::sha256_file_hex(&src).expect("hex"));
        assert_eq!(std::fs::read(&dest).expect("read dest"), b"payload");
    }

    #[test]
    #[cfg(unix)]
    fn copy_verified_restages_a_read_only_source() {
        let scratch = scratch_dir();
        for (src_mode, want_mode) in [(0o555, 0o755), (0o444, 0o644)] {
            let src = scratch.path().join(format!("src-{src_mode:o}.bin"));
            std::fs::write(&src, b"payload").expect("write src");
            set_mode(&src, src_mode);
            let dest = scratch.path().join(format!("dest-{src_mode:o}.bin"));
            let first = copy_verified(&src, &dest).expect("first copy");
            let second = copy_verified(&src, &dest).expect("second copy");
            assert_eq!(first, second);
            assert_eq!(std::fs::read(&dest).expect("read dest"), b"payload");
            assert_eq!(mode_bits(&dest), want_mode);
        }
    }

    #[test]
    #[cfg(unix)]
    fn archive_stage_runs_twice_into_one_outdir() {
        let scratch = scratch_dir();
        let app = scratch.path().join("app_name.txt");
        std::fs::write(&app, b"release_demo_app\n").expect("write app");
        let tarball = scratch.path().join("release_demo.tar.gz");
        std::fs::write(&tarball, b"archive-tarball-bytes-v1").expect("write tarball");
        let digest = dx_digest::sha256_file_hex(&tarball).expect("digest");
        let checksum = scratch.path().join("release_demo.tar.gz.sha256");
        std::fs::write(
            &checksum,
            format!("{digest}  release_demo.tar.gz\n").as_bytes(),
        )
        .expect("write checksum");
        set_mode(&tarball, 0o444);
        set_mode(&checksum, 0o444);
        let outdir = scratch.path().join("out");
        let first = archive_stage_release(&app, &tarball, &checksum, &outdir).expect("first stage");
        let second =
            archive_stage_release(&app, &tarball, &checksum, &outdir).expect("second stage");
        assert_eq!(first, second);
        assert_eq!(
            std::fs::read(&second.1).expect("read tarball"),
            b"archive-tarball-bytes-v1"
        );
        assert_eq!(mode_bits(&second.1), 0o644);
        assert_eq!(mode_bits(&second.2), 0o644);
    }

    #[test]
    fn which_on_path_finds_present_and_missing_tools() {
        if env_test_in_child("tests::which_on_path_finds_present_and_missing_tools") {
            return;
        }
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
    fn deploy_outdir_prefers_arg_then_env_then_cwd() {
        if env_test_in_child("tests::deploy_outdir_prefers_arg_then_env_then_cwd") {
            return;
        }
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
        let relative = vec!["prog".to_owned(), "dist".to_owned()];
        let invocation = scratch.path().join("inv");
        with_env(
            &[(
                "BUILD_WORKING_DIRECTORY",
                Some(invocation.to_string_lossy().as_ref()),
            )],
            || {
                assert_eq!(deploy_outdir(&relative), invocation.join("dist"));
                assert_eq!(deploy_outdir(&argv), explicit);
            },
        );
        with_env(&[("BUILD_WORKING_DIRECTORY", None)], || {
            let cwd = std::env::current_dir().expect("cwd");
            assert_eq!(deploy_outdir(&relative), cwd.join("dist"));
        });
        with_env(&[("BUILD_WORKING_DIRECTORY", Some(""))], || {
            let cwd = std::env::current_dir().expect("cwd");
            assert_eq!(deploy_outdir(&relative), cwd.join("dist"));
        });
    }

    #[test]
    fn resolve_runfile_reads_dir_and_manifest() {
        if env_test_in_child("tests::resolve_runfile_reads_dir_and_manifest") {
            return;
        }
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
        if env_test_in_child("tests::npm_find_pack_inputs_reads_manifest_entries") {
            return;
        }
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
                "_main/npm/nested/npm_demo.feed.json {}\n_main/npm/nested/npm_demo.tgz {}\n",
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
                let (found_feed, found_tgz) = npm_find_pack_inputs(
                    "_main/npm/nested/npm_demo.feed.json",
                    "_main/npm/nested/npm_demo.tgz",
                )
                .expect("pack pair");
                assert_eq!(found_feed, feed);
                assert_eq!(found_tgz, tgz);
                let error = npm_find_pack_inputs("npm_demo.feed.json", "npm_demo.tgz")
                    .expect_err("basename is not a key");
                assert_eq!(error.kind(), io::ErrorKind::NotFound);
                assert!(error.to_string().contains("npm_demo.feed.json"));
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
                let error = npm_find_pack_inputs("_main/a.feed.json", "_main/a.tgz")
                    .expect_err("no feed under runfiles");
                assert_eq!(error.kind(), io::ErrorKind::NotFound);
            },
        );
    }

    #[test]
    fn resolve_runfile_reads_manifest_values_with_spaces() {
        if env_test_in_child("tests::resolve_runfile_reads_manifest_values_with_spaces") {
            return;
        }
        let scratch = scratch_dir();
        let payload = scratch.path().join("real dir").join("app.bin");
        std::fs::create_dir_all(payload.parent().expect("parent")).expect("nested");
        std::fs::write(&payload, b"spaced").expect("write payload");
        let manifest = scratch.path().join("spaced.manifest");
        std::fs::write(
            &manifest,
            format!("_main/pkg/app.bin {}\n", payload.display()),
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
                let found = resolve_runfile("_main/pkg/app.bin").expect("spaced value");
                assert_eq!(std::fs::read(found).expect("bytes"), b"spaced");
                let error = resolve_runfile("_main/pkg/absent.bin").expect_err("missing key");
                assert_eq!(error.kind(), io::ErrorKind::NotFound);
                assert!(error.to_string().contains("_main/pkg/absent.bin"));
            },
        );
    }

    #[test]
    fn resolve_runfile_reads_escaped_manifest_entries() {
        if env_test_in_child("tests::resolve_runfile_reads_escaped_manifest_entries") {
            return;
        }
        let scratch = scratch_dir();
        let spaced = scratch.path().join("real dir").join("a b.bin");
        std::fs::create_dir_all(spaced.parent().expect("parent")).expect("nested");
        std::fs::write(&spaced, b"spaced").expect("write spaced");
        let slashed = scratch.path().join("slash.bin");
        std::fs::write(&slashed, b"slashed").expect("write slashed");
        let manifest = scratch.path().join("escaped.manifest");
        std::fs::write(
            &manifest,
            format!(
                " _main/pkg/a\\sb.bin {}\n _main/pkg/c\\bd\\ne {}\n",
                spaced.display(),
                slashed.display()
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
                let found = resolve_runfile("_main/pkg/a b.bin").expect("escaped space");
                assert_eq!(std::fs::read(found).expect("bytes"), b"spaced");
                let found = resolve_runfile("_main/pkg/c\\d\ne").expect("escaped slash");
                assert_eq!(std::fs::read(found).expect("bytes"), b"slashed");
                let error = resolve_runfile("_main/pkg/a\\sb.bin").expect_err("raw escape");
                assert_eq!(error.kind(), io::ErrorKind::NotFound);
            },
        );
    }

    #[test]
    fn resolve_runfile_maps_a_repository_name_through_the_manifest() {
        if env_test_in_child("tests::resolve_runfile_maps_a_repository_name_through_the_manifest") {
            return;
        }
        let scratch = scratch_dir();
        let payload = scratch.path().join("dep.bin");
        std::fs::write(&payload, b"external").expect("write payload");
        let mapping = scratch.path().join("repo_mapping");
        std::fs::write(&mapping, ",dep,dep+1.0.0+\n").expect("write mapping");
        let manifest = scratch.path().join("mapped.manifest");
        std::fs::write(
            &manifest,
            format!(
                "_repo_mapping {}\ndep+1.0.0+/pkg/app.bin {}\n",
                mapping.display(),
                payload.display()
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
                let found = resolve_runfile("dep/pkg/app.bin").expect("mapped name");
                assert_eq!(std::fs::read(found).expect("bytes"), b"external");
                let error = resolve_runfile("other/pkg/app.bin").expect_err("unmapped name");
                assert_eq!(error.kind(), io::ErrorKind::NotFound);
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
        let parent = path.parent().expect("npmrc parent");
        assert_ne!(parent, std::env::temp_dir().as_path());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(parent)
                .expect("stat")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o700, "npmrc dir must stay owner-only");
        }
        std::fs::remove_dir_all(parent).expect("cleanup");
    }

    #[test]
    fn secure_file_is_owner_only_and_refuses_existing() {
        let scratch = scratch_dir();
        let path = write_secure_file(scratch.path(), "secret", "one").expect("secure");
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "one");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&path).expect("stat").permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "secret must stay owner-only");
        }
        let error = write_secure_file(scratch.path(), "secret", "two").expect_err("existing");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "one");
        #[cfg(unix)]
        {
            let target = scratch.path().join("target");
            std::fs::write(&target, "keep").expect("write target");
            std::os::unix::fs::symlink(&target, scratch.path().join("link")).expect("symlink");
            assert!(write_secure_file(scratch.path(), "link", "leak").is_err());
            assert_eq!(
                std::fs::read_to_string(&target).expect("read target"),
                "keep"
            );
        }
    }

    #[test]
    fn private_dir_refuses_existing_dir() {
        let scratch = scratch_dir();
        let error = create_private_dir(scratch.path()).expect_err("existing");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        let child = scratch.path().join("child");
        create_private_dir(&child).expect("child");
        assert!(child.is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&child)
                .expect("stat")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o700, "private dir must stay owner-only");
        }
    }

    #[test]
    fn promotion_secret_refs_cover_every_ref_kind() {
        if env_test_in_child("tests::promotion_secret_refs_cover_every_ref_kind") {
            return;
        }
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
        if env_test_in_child("tests::promotion_health_cmd_rejects_empty_and_missing_tools") {
            return;
        }
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
            PromotionHealth::SkippedLocal,
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
    fn npm_access_error_covers_scope_and_mode() {
        assert!(npm_access_error("npm-demo", "public").is_none());
        assert!(npm_access_error("@s/npm-demo", "public").is_none());
        assert!(npm_access_error("@s/npm-demo", "restricted").is_none());
        let unscoped = npm_access_error("npm-demo", "restricted").expect("unscoped");
        assert!(unscoped.contains("needs a scoped package"), "{unscoped}");
        let unknown = npm_access_error("npm-demo", "private").expect("unknown");
        assert!(unknown.contains("invalid access"), "{unknown}");
        for bad in ["@scope", "@/name", "@a/b/c", "a/b"] {
            assert!(npm_access_error(bad, "public").is_some(), "{bad}");
        }
    }

    #[test]
    fn npm_publish_args_carry_access_and_no_secret() {
        let tgz = Path::new("/stage/p.tgz");
        let public = npm_publish_args(tgz, "latest", "public", "https://r.example", "required");
        let public: Vec<String> = public
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            public,
            [
                "publish",
                "/stage/p.tgz",
                "--tag",
                "latest",
                "--access",
                "public",
                "--provenance",
                "--registry",
                "https://r.example"
            ]
        );
        let restricted = npm_publish_args(tgz, "next", "restricted", "https://r.example", "none");
        let restricted: Vec<String> = restricted
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(restricted.contains(&"restricted".to_owned()));
        assert!(!restricted.contains(&"--provenance".to_owned()));
        assert!(restricted.contains(&"--provenance=false".to_owned()));
    }

    #[test]
    fn npm_pack_rejects_restricted_unscoped_before_writing() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(&src, b"{\"name\": \"npm-demo\", \"version\": \"0.0.0\"}\n").expect("write");
        let tgz = scratch.path().join("out.tgz");
        let feed = scratch.path().join("out.feed.json");
        let error = npm_create_pack(
            "npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "restricted",
            "none",
            &tgz,
            &feed,
            &[src],
        )
        .expect_err("mismatch");
        assert!(
            error.to_string().contains("needs a scoped package"),
            "{error}"
        );
        assert!(!tgz.exists() && !feed.exists());
    }

    #[test]
    fn npm_verify_pack_reports_restricted_access() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(
            &src,
            b"{\"name\": \"@s/npm-demo\", \"version\": \"0.0.0\"}\n",
        )
        .expect("write");
        let tgz = scratch.path().join("out.tgz");
        let feed = scratch.path().join("out.feed.json");
        npm_create_pack(
            "@s/npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "restricted",
            "none",
            &tgz,
            &feed,
            &[src],
        )
        .expect("pack");
        let (name, _, registry, access, _) = npm_verify_pack(&tgz, &feed).expect("verify");
        assert_eq!(name, "@s/npm-demo");
        assert_eq!(registry, "https://registry.npmjs.org");
        assert_eq!(access, "restricted");
        let body = std::fs::read_to_string(&feed).expect("feed");
        std::fs::write(&feed, body.replace("\"@s/npm-demo\"", "\"npm-demo\"")).expect("tamper");
        let error = npm_verify_pack(&tgz, &feed).expect_err("tampered");
        assert!(
            error.to_string().contains("needs a scoped package"),
            "{error}"
        );
    }

    #[test]
    fn npm_deploy_rejects_former_dry_run_request() {
        with_env(&[("NPM_PUBLISH_DRY_RUN", Some("1"))], || {
            assert_eq!(
                npm_deploy_main(&["prog".to_owned()], "a.feed.json", "a.tgz"),
                1
            );
        });
    }

    #[test]
    fn npm_cli_pack_and_publish_dry_run_accept_the_staged_package() {
        let ok = |tool: &str| {
            std::process::Command::new(tool)
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        };
        if !ok("npm") {
            return;
        }
        let scratch = scratch_dir();
        let pkg = scratch.path().join("pkg");
        std::fs::create_dir_all(&pkg).expect("pkg");
        std::fs::write(
            pkg.join("package.json"),
            b"{\"name\": \"@s/npm-demo\", \"version\": \"0.0.0\"}\n",
        )
        .expect("write");
        let tgz = scratch.path().join("out.tgz");
        let feed = scratch.path().join("out.feed.json");
        npm_create_pack(
            "@s/npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "restricted",
            "none",
            &tgz,
            &feed,
            &[pkg.join("package.json")],
        )
        .expect("pack");
        let run = |args: Vec<std::ffi::OsString>| {
            std::process::Command::new("npm")
                .args(args)
                .env("npm_config_cache", scratch.path().join("npm-cache"))
                .env("npm_config_userconfig", scratch.path().join("npmrc"))
                .env("npm_config_update_notifier", "false")
                .current_dir(scratch.path())
                .output()
                .expect("npm")
        };
        let packed = run(vec![
            "pack".into(),
            "--dry-run".into(),
            "--json".into(),
            pkg.clone().into(),
        ]);
        assert!(packed.status.success(), "{packed:?}");
        let mut args = npm_publish_args(
            &tgz,
            "latest",
            "restricted",
            "https://registry.npmjs.org",
            "none",
        );
        args.push("--dry-run".into());
        let published = run(args);
        let text = String::from_utf8_lossy(&published.stderr).into_owned()
            + &String::from_utf8_lossy(&published.stdout);
        assert!(published.status.success(), "{text}");
        assert!(text.contains("@s/npm-demo"), "{text}");
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
            nuget_main("a", "b", "", "", "https://nuget.invalid", &argv),
            1
        );
        assert_eq!(crates_main("a", "b", "1.0.0", &argv), 1);
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
            oci_main(
                "a",
                "registry.invalid",
                "demo/app",
                "1.0.0",
                "linux/amd64",
                &argv
            ),
            1
        );
        assert_eq!(octopus_main(&spec, &argv), 1);
        assert_eq!(
            promotion_main("a", "b", "staging", "production", "1.0.0", &argv),
            1
        );
        assert_eq!(npm_deploy_main(&argv, "a", "b"), 1);
    }

    #[test]
    fn launchers_report_missing_pinned_inputs() {
        if env_test_in_child("tests::launchers_report_missing_pinned_inputs") {
            return;
        }
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
                    nuget_main(missing, "demo", "", "", "https://nuget.invalid", &argv),
                    1
                );
                assert_eq!(crates_main(missing, "demo", "1.0.0", &argv), 1);
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
                    oci_main(
                        missing,
                        "registry.invalid",
                        "demo/app",
                        "1.0.0",
                        "linux/amd64",
                        &argv
                    ),
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

    fn oci_tar(path: &Path, files: &[(&str, &[u8])], links: &[(&str, &str)]) {
        let mut builder = tar::Builder::new(Vec::new());
        for (name, body) in files {
            let header = ustar_header(name, body.len() as u64, 0o644).expect("header");
            builder.append(&header, *body).expect("append");
        }
        for (name, target) in links {
            let mut header = tar::Header::new_ustar();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_mode(0o777);
            header.set_path(name).expect("link path");
            header.set_link_name(target).expect("link target");
            header.set_cksum();
            builder.append(&header, io::empty()).expect("append link");
        }
        std::fs::write(path, builder.into_inner().expect("tar")).expect("write archive");
    }

    fn oci_save(
        dir: &Path,
        name: &str,
        layers: &[Vec<u8>],
        platform: &str,
    ) -> (std::path::PathBuf, String) {
        let mut paths = Vec::new();
        for (index, bytes) in layers.iter().enumerate() {
            let path = dir.join(format!("{name}-layer-{index}"));
            std::fs::write(&path, bytes).expect("write layer");
            paths.push(path);
        }
        let refs: Vec<&Path> = paths.iter().map(|path| path.as_path()).collect();
        let archive = dir.join(name);
        let id = archive_image(&refs, platform, &["/app/hello"], &archive).expect("archive image");
        if name.ends_with(".gz") {
            let tar = std::fs::read(&archive).expect("read archive");
            std::fs::write(&archive, gzip_compress(&tar).expect("gzip")).expect("write gz");
        }
        (archive, id)
    }

    fn oci_config(architecture: &str, diff_ids: &[&[u8]]) -> Vec<u8> {
        let ids: Vec<String> = diff_ids
            .iter()
            .map(|tar| format!("sha256:{}", dx_digest::sha256_hex(tar)))
            .collect();
        serde_json::to_vec(&serde_json::json!({
            "architecture": architecture,
            "config": {"Entrypoint": ["/app/hello"], "Env": ["PATH=/usr/bin", "DEMO=1"]},
            "os": "linux",
            "rootfs": {"diff_ids": ids, "type": "layers"},
        }))
        .expect("config")
    }

    #[test]
    fn oci_declared_image_id_binds_the_single_config_digest() {
        let scratch = scratch_dir();
        let archive = scratch.path().join("image.tar");
        let layer = oci_test_layer(false);
        let config = oci_config("amd64", &[&layer]);
        oci_tar(
            &archive,
            &[
                (
                    "manifest.json",
                    b"[{\"Config\":\"abc.json\",\"RepoTags\":[\"other:tag\"],\"Layers\":[\"abc/layer.tar\"]}]",
                ),
                ("abc.json", &config),
                ("abc/layer.tar", &layer),
            ],
            &[],
        );
        assert_eq!(
            oci_declared_image_id(&archive, "linux/amd64").expect("id"),
            format!("sha256:{}", dx_digest::sha256_hex(&config))
        );
        let (saved, id) = oci_save(
            scratch.path(),
            "saved.tar",
            &[oci_test_layer(false), oci_test_layer(true)],
            "linux/amd64",
        );
        assert_eq!(
            oci_declared_image_id(&saved, "linux/amd64").expect("saved id"),
            id
        );
    }

    #[test]
    fn oci_declared_image_id_rejects_unsupported_archives() {
        let scratch = scratch_dir();
        let archive = scratch.path().join("image.tar");
        let layer = oci_test_layer(false);
        let config = oci_config("amd64", &[&layer]);
        oci_tar(&archive, &[("manifest.json", b"[]")], &[]);
        assert!(oci_declared_image_id(&archive, "linux/amd64")
            .expect_err("empty")
            .contains("exactly one"));
        oci_tar(
            &archive,
            &[
                (
                    "manifest.json",
                    b"[{\"Config\":\"a.json\",\"Layers\":[\"l.tar\"]},{\"Config\":\"a.json\",\"Layers\":[\"l.tar\"]}]",
                ),
                ("a.json", &config),
                ("l.tar", &layer),
            ],
            &[],
        );
        assert!(oci_declared_image_id(&archive, "linux/amd64")
            .expect_err("many")
            .contains("2 images"));
        oci_tar(
            &archive,
            &[
                (
                    "manifest.json",
                    b"[{\"Config\":\"gone.json\",\"Layers\":[\"l.tar\"]}]",
                ),
                ("a.json", &config),
                ("l.tar", &layer),
            ],
            &[],
        );
        assert!(oci_declared_image_id(&archive, "linux/amd64")
            .expect_err("missing config")
            .contains("missing its config"));
        oci_tar(
            &archive,
            &[
                ("manifest.json", b"[{\"Config\":\"a.json\"}]"),
                ("a.json", &config),
            ],
            &[],
        );
        assert!(oci_declared_image_id(&archive, "linux/amd64")
            .expect_err("no layers")
            .contains("missing Layers"));
        oci_tar(&archive, &[("manifest.json", b"{not json")], &[]);
        assert!(oci_declared_image_id(&archive, "linux/amd64")
            .expect_err("malformed")
            .contains("malformed manifest.json"));
        std::fs::write(&archive, b"image").expect("rootfs");
        assert!(oci_declared_image_id(&archive, "linux/amd64").is_err());
        let layer = {
            let mut builder = tar::Builder::new(Vec::new());
            let mut header = ustar_header("etc/os-release", 2, 0o644).expect("header");
            header.set_cksum();
            builder.append(&header, &b"x\n"[..]).expect("append");
            builder.into_inner().expect("tar")
        };
        std::fs::write(&archive, layer).expect("layer");
        assert!(oci_declared_image_id(&archive, "linux/amd64")
            .expect_err("rootfs layer")
            .contains("no manifest.json or index.json"));
    }

    fn oci_digest(bytes: &[u8]) -> String {
        format!("sha256:{}", dx_digest::sha256_hex(bytes))
    }

    fn oci_blob(bytes: &[u8]) -> String {
        format!("blobs/sha256/{}", dx_digest::sha256_hex(bytes))
    }

    fn oci_descriptor(media_type: &str, body: &[u8]) -> serde_json::Value {
        serde_json::json!({
            "digest": oci_digest(body),
            "mediaType": media_type,
            "size": body.len(),
        })
    }

    fn oci_index_body(manifests: &[serde_json::Value]) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "manifests": manifests,
            "mediaType": "application/vnd.oci.image.index.v1+json",
            "schemaVersion": 2,
        }))
        .expect("index")
    }

    fn oci_manifest_body(config: &[u8], layers: &[serde_json::Value]) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "config": oci_descriptor("application/vnd.oci.image.config.v1+json", config),
            "layers": layers,
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "schemaVersion": 2,
        }))
        .expect("manifest")
    }

    fn oci_layer_descriptor(layer: &[u8]) -> serde_json::Value {
        let media_type = if layer.starts_with(&GZIP_MAGIC) {
            "application/vnd.oci.image.layer.v1.tar+gzip"
        } else {
            "application/vnd.oci.image.layer.v1.tar"
        };
        oci_descriptor(media_type, layer)
    }

    struct OciIndexFixture {
        config: Vec<u8>,
        manifest: Vec<u8>,
        members: Vec<(String, Vec<u8>)>,
    }

    fn oci_index_fixture(layers: &[Vec<u8>], architecture: &str, nest: bool) -> OciIndexFixture {
        let uncompressed: Vec<Vec<u8>> = layers
            .iter()
            .map(|layer| {
                if layer.starts_with(&GZIP_MAGIC) {
                    let mut tar = Vec::new();
                    flate2::read::MultiGzDecoder::new(layer.as_slice())
                        .read_to_end(&mut tar)
                        .expect("gunzip");
                    tar
                } else {
                    layer.clone()
                }
            })
            .collect();
        let diff_ids: Vec<&[u8]> = uncompressed.iter().map(Vec::as_slice).collect();
        let config = oci_config(architecture, &diff_ids);
        let descriptors: Vec<serde_json::Value> = layers
            .iter()
            .map(|layer| oci_layer_descriptor(layer))
            .collect();
        let manifest = oci_manifest_body(&config, &descriptors);
        let mut image = oci_descriptor("application/vnd.oci.image.manifest.v1+json", &manifest);
        image["platform"] = serde_json::json!({"architecture": architecture, "os": "linux"});
        let mut members = vec![
            (
                "oci-layout".to_owned(),
                b"{\"imageLayoutVersion\":\"1.0.0\"}".to_vec(),
            ),
            (oci_blob(&manifest), manifest.clone()),
            (oci_blob(&config), config.clone()),
        ];
        for layer in layers {
            members.push((oci_blob(layer), layer.clone()));
        }
        let top = if nest {
            let statement = b"{\"_type\":\"https://in-toto.io/Statement/v0.1\"}".to_vec();
            let attestation = oci_manifest_body(
                b"{}",
                &[oci_descriptor("application/vnd.in-toto+json", &statement)],
            );
            let mut skipped =
                oci_descriptor("application/vnd.oci.image.manifest.v1+json", &attestation);
            skipped["annotations"] = serde_json::json!({
                "vnd.docker.reference.digest": oci_digest(&manifest),
                "vnd.docker.reference.type": "attestation-manifest",
            });
            let inner = oci_index_body(&[image, skipped]);
            members.push((oci_blob(&attestation), attestation));
            members.push((oci_blob(&statement), statement));
            members.push((oci_blob(&inner), inner.clone()));
            oci_descriptor("application/vnd.oci.image.index.v1+json", &inner)
        } else {
            image
        };
        members.push(("index.json".to_owned(), oci_index_body(&[top])));
        OciIndexFixture {
            config,
            manifest,
            members,
        }
    }

    fn oci_index_tar(
        path: &Path,
        layers: &[Vec<u8>],
        architecture: &str,
        nest: bool,
        extra: &[(&str, &[u8])],
        links: &[(&str, &str)],
    ) -> OciIndexFixture {
        let fixture = oci_index_fixture(layers, architecture, nest);
        let mut members: Vec<(String, Vec<u8>)> = fixture
            .members
            .iter()
            .filter(|(name, _)| {
                !extra.iter().any(|(other, _)| other == name)
                    && !links.iter().any(|(other, _)| other == name)
            })
            .cloned()
            .collect();
        members.extend(
            extra
                .iter()
                .map(|(name, body)| ((*name).to_owned(), body.to_vec())),
        );
        let files: Vec<(&str, &[u8])> = members
            .iter()
            .map(|(name, body)| (name.as_str(), body.as_slice()))
            .collect();
        oci_tar(path, &files, links);
        if path.to_string_lossy().ends_with(".gz") {
            let tar = std::fs::read(path).expect("read archive");
            std::fs::write(path, gzip_compress(&tar).expect("gzip")).expect("write gz");
        }
        fixture
    }

    #[test]
    fn oci_archive_image_id_reads_oci_layout_archives() {
        let scratch = scratch_dir();
        let layers = [oci_test_layer(false), oci_test_layer(true)];
        for (name, nest) in [
            ("flat.tar", false),
            ("nested.tar", true),
            ("flat.tar.gz", false),
            ("nested.tar.gz", true),
        ] {
            let archive = scratch.path().join(name);
            let fixture = oci_index_tar(&archive, &layers, "amd64", nest, &[], &[]);
            assert_eq!(
                oci_declared_image_id(&archive, "linux/amd64").expect(name),
                oci_digest(&fixture.config),
                "{name}"
            );
        }
        let archive = scratch.path().join("linked.tar");
        let fixture = oci_index_fixture(&layers, "amd64", true);
        let manifest_blob = oci_blob(&fixture.manifest);
        let config_blob = oci_blob(&fixture.config);
        let layer_blob = oci_blob(&layers[1]);
        oci_index_tar(
            &archive,
            &layers,
            "amd64",
            true,
            &[
                ("store/manifest.json", &fixture.manifest),
                ("store/config.json", &fixture.config),
                ("store/layer.tar.gz", &layers[1]),
            ],
            &[
                (&manifest_blob, "../../store/manifest.json"),
                (&config_blob, "../../store/config.json"),
                (&layer_blob, "../../store/layer.tar.gz"),
            ],
        );
        assert_eq!(
            oci_declared_image_id(&archive, "linux/amd64").expect("linked"),
            oci_digest(&fixture.config)
        );
        let fixture = oci_index_fixture(&layers, "amd64", true);
        let docker_manifest = serde_json::to_vec(&serde_json::json!([{
            "Config": oci_blob(&fixture.config),
            "Layers": layers.iter().map(|layer| oci_blob(layer)).collect::<Vec<_>>(),
            "RepoTags": ["demo/app:1.0.0"],
        }]))
        .expect("docker manifest");
        for (name, index) in [
            ("docker25.tar", None),
            ("docker25_index_ignored.tar", Some(b"{not json".as_slice())),
        ] {
            let archive = scratch.path().join(name);
            let mut extra: Vec<(&str, &[u8])> = vec![("manifest.json", docker_manifest.as_slice())];
            if let Some(index) = index {
                extra.push(("index.json", index));
            }
            oci_index_tar(&archive, &layers, "amd64", true, &extra, &[]);
            assert_eq!(
                oci_declared_image_id(&archive, "linux/amd64").expect(name),
                oci_digest(&fixture.config),
                "{name}"
            );
        }
    }

    #[test]
    fn oci_archive_image_id_rejects_unsupported_oci_layout_archives() {
        let scratch = scratch_dir();
        let archive = scratch.path().join("image.tar");
        let layers = [oci_test_layer(false)];
        let fixture = oci_index_fixture(&layers, "amd64", false);
        let image = oci_descriptor(
            "application/vnd.oci.image.manifest.v1+json",
            &fixture.manifest,
        );
        let config_blob = oci_blob(&fixture.config);
        let layer_blob = oci_blob(&layers[0]);
        let other_config = oci_config("amd64", &[&oci_test_layer(true)]);
        let other_layer = oci_test_layer(true);
        let missing = oci_descriptor("application/vnd.oci.image.manifest.v1+json", b"gone");
        let with_digest = |digest: &str| {
            let mut descriptor = image.clone();
            descriptor["digest"] = serde_json::json!(digest);
            oci_index_body(&[descriptor])
        };
        let other = oci_descriptor("application/vnd.oci.image.manifest.v1+json", b"other");
        let two = oci_index_body(&[image.clone(), other]);
        let gone = oci_index_body(&[missing]);
        let md5 = with_digest("md5:0123456789abcdef0123456789abcdef");
        let escape = with_digest("sha256:../x");
        let upper = with_digest(
            &oci_digest(&fixture.manifest)
                .to_uppercase()
                .replace("SHA256", "sha256"),
        );
        let mut foreign = image.clone();
        foreign["mediaType"] = serde_json::json!("application/vnd.example.manifest+json");
        let foreign = oci_index_body(&[foreign]);
        let manifest_with = |layers: &[serde_json::Value]| {
            let manifest = oci_manifest_body(&fixture.config, layers);
            let index = oci_index_body(&[oci_descriptor(
                "application/vnd.oci.image.manifest.v1+json",
                &manifest,
            )]);
            (oci_blob(&manifest), manifest, index)
        };
        let mut zstd = oci_layer_descriptor(&layers[0]);
        zstd["mediaType"] = serde_json::json!("application/vnd.oci.image.layer.v1.tar+zstd");
        let zstd = manifest_with(&[zstd]);
        let mut nondistributable = oci_layer_descriptor(&layers[0]);
        nondistributable["mediaType"] =
            serde_json::json!("application/vnd.oci.image.layer.nondistributable.v1.tar+gzip");
        let nondistributable = manifest_with(&[nondistributable]);
        let empty = manifest_with(&[]);
        let mut deep: Vec<(String, Vec<u8>)> = Vec::new();
        let mut top = image.clone();
        for _ in 0..=OCI_INDEX_DEPTH {
            let body = oci_index_body(&[top]);
            top = oci_descriptor("application/vnd.oci.image.index.v1+json", &body);
            deep.push((oci_blob(&body), body));
        }
        deep.push(("index.json".to_owned(), oci_index_body(&[top])));
        let deep: Vec<(&str, &[u8])> = deep
            .iter()
            .map(|(name, body)| (name.as_str(), body.as_slice()))
            .collect();
        let manifest_blob = oci_blob(&fixture.manifest);
        let cases: Vec<(&str, Vec<(&str, &[u8])>, &str)> = vec![
            (
                "two images",
                vec![("index.json", two.as_slice())],
                "2 images",
            ),
            (
                "malformed",
                vec![("index.json", &b"{not json"[..])],
                "malformed index.json",
            ),
            (
                "no manifests",
                vec![("index.json", &b"{}"[..])],
                "malformed index.json",
            ),
            (
                "missing manifest",
                vec![("index.json", gone.as_slice())],
                "is missing its blob 'sha256:",
            ),
            (
                "manifest mismatch",
                vec![(manifest_blob.as_str(), &b"{}"[..])],
                "does not match its digest",
            ),
            (
                "config mismatch",
                vec![(config_blob.as_str(), other_config.as_slice())],
                "does not match its digest",
            ),
            (
                "layer mismatch",
                vec![(layer_blob.as_str(), other_layer.as_slice())],
                "does not match its digest",
            ),
            (
                "md5",
                vec![("index.json", md5.as_slice())],
                "not a sha256 digest",
            ),
            (
                "escape",
                vec![("index.json", escape.as_slice())],
                "not a sha256 digest",
            ),
            (
                "uppercase",
                vec![("index.json", upper.as_slice())],
                "not a sha256 digest",
            ),
            (
                "zstd",
                vec![
                    ("index.json", zstd.2.as_slice()),
                    (zstd.0.as_str(), zstd.1.as_slice()),
                ],
                "unsupported layer media type 'application/vnd.oci.image.layer.v1.tar+zstd'",
            ),
            (
                "nondistributable",
                vec![
                    ("index.json", nondistributable.2.as_slice()),
                    (nondistributable.0.as_str(), nondistributable.1.as_slice()),
                ],
                "unsupported layer media type",
            ),
            (
                "empty layers",
                vec![
                    ("index.json", empty.2.as_slice()),
                    (empty.0.as_str(), empty.1.as_slice()),
                ],
                "malformed image manifest: missing layers",
            ),
            ("too deep", deep, "deeper than 8 levels"),
            (
                "foreign manifest",
                vec![("index.json", foreign.as_slice())],
                "unsupported manifest media type 'application/vnd.example.manifest+json'",
            ),
        ];
        for (case, extra, want) in cases {
            oci_index_tar(&archive, &layers, "amd64", false, &extra, &[]);
            let error = oci_declared_image_id(&archive, "linux/amd64").expect_err(case);
            assert!(error.contains(want), "{case}: {error}");
        }
        oci_index_tar(&archive, &layers, "amd64", false, &[], &[]);
        assert_eq!(
            oci_declared_image_id(&archive, "linux/amd64").expect("valid"),
            oci_digest(&fixture.config)
        );
        // The same manifest under two tags, or twice in a nested index, is one image.
        let mut tagged = Vec::new();
        for tag in ["one", "two"] {
            let mut descriptor = image.clone();
            descriptor["annotations"] =
                serde_json::json!({"org.opencontainers.image.ref.name": tag});
            tagged.push(descriptor);
        }
        let tagged_index = oci_index_body(&tagged);
        let nested_body = oci_index_body(&[image.clone(), image.clone()]);
        let nested_blob = oci_blob(&nested_body);
        let nested_index = oci_index_body(&[oci_descriptor(
            "application/vnd.oci.image.index.v1+json",
            &nested_body,
        )]);
        let positives: Vec<(&str, Vec<(&str, &[u8])>)> = vec![
            ("two tags", vec![("index.json", tagged_index.as_slice())]),
            (
                "nested twice",
                vec![
                    ("index.json", nested_index.as_slice()),
                    (nested_blob.as_str(), nested_body.as_slice()),
                ],
            ),
        ];
        for (case, extra) in positives {
            oci_index_tar(&archive, &layers, "amd64", false, &extra, &[]);
            assert_eq!(
                oci_declared_image_id(&archive, "linux/amd64").expect(case),
                oci_digest(&fixture.config),
                "{case}"
            );
        }
    }

    #[test]
    fn oci_layout_reads_oci_layout_archives() {
        let scratch = scratch_dir();
        let layers = [oci_test_layer(false), oci_test_layer(true)];
        let index_tar = scratch.path().join("index.tar");
        let fixture = oci_index_tar(&index_tar, &layers, "amd64", true, &[], &[]);
        let docker_manifest = serde_json::to_vec(&serde_json::json!([{
            "Config": "config.json",
            "Layers": ["0/layer.tar", "1/layer.tar"],
        }]))
        .expect("docker manifest");
        let docker_tar = scratch.path().join("docker.tar");
        oci_tar(
            &docker_tar,
            &[
                ("manifest.json", &docker_manifest),
                ("config.json", &fixture.config),
                ("0/layer.tar", &layers[0]),
                ("1/layer.tar", &layers[1]),
            ],
            &[],
        );
        let mut built = Vec::new();
        for (name, image) in [("index", &index_tar), ("docker", &docker_tar)] {
            let outdir = scratch.path().join(name);
            std::fs::create_dir_all(&outdir).expect("outdir");
            let layout = oci_build_layout(
                image,
                &outdir,
                "registry.invalid",
                "demo/app",
                "1.0.0",
                "linux/amd64",
            )
            .expect(name);
            built.push(oci_layout_json(&layout));
            for layer in &layers {
                let stored = layout
                    .join("blobs")
                    .join("sha256")
                    .join(dx_digest::sha256_hex(layer));
                assert_eq!(std::fs::read(stored).expect("blob"), *layer, "{name}");
            }
        }
        let (manifest, config) = &built[0];
        assert_eq!(built[0], built[1]);
        assert_eq!(
            manifest["config"]["digest"],
            oci_digest(&fixture.config).as_str()
        );
        let digests: Vec<String> = layers.iter().map(|layer| oci_digest(layer)).collect();
        assert_eq!(manifest["layers"][0]["digest"], digests[0].as_str());
        assert_eq!(manifest["layers"][1]["digest"], digests[1].as_str());
        assert_eq!(
            manifest["layers"][0]["mediaType"],
            "application/vnd.oci.image.layer.v1.tar"
        );
        assert_eq!(
            manifest["layers"][1]["mediaType"],
            "application/vnd.oci.image.layer.v1.tar+gzip"
        );
        assert_eq!(config["architecture"], "amd64");
        let arm = scratch.path().join("arm.tar");
        oci_index_tar(&arm, &layers, "arm64", false, &[], &[]);
        let outdir = scratch.path().join("arm");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let error = oci_build_layout(
            &arm,
            &outdir,
            "registry.invalid",
            "demo/app",
            "1.0.0",
            "linux/amd64",
        )
        .expect_err("arm64 config");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(
            error.to_string().contains("image config is 'linux/arm64'"),
            "{error}"
        );
        assert!(std::fs::read_dir(&outdir).expect("outdir").next().is_none());
    }

    /// Quotes one value for a single-quoted sh word.
    #[cfg(unix)]
    fn sh_quote(value: &str) -> String {
        format!("'{}'", value.replace('\'', "'\\''"))
    }

    /// Writes a fake docker with its settings baked in, since docker runs in a cleared env.
    #[cfg(unix)]
    fn fake_docker(dir: &Path, settings: &[(&str, &str)]) {
        use std::os::unix::fs::PermissionsExt;
        let script = dir.join("docker");
        let mut text = "#!/bin/sh\n".to_owned();
        for (key, value) in settings {
            text.push_str(&format!("{key}={}\n", sh_quote(value)));
        }
        text.push_str(
            "echo \"$*\" >> \"$FAKE_DOCKER_LOG\"\n\
             echo \"$1|$DOCKER_CONFIG|${DOCKER_HOST-unset}|${DOCKER_CONTEXT-unset}|${DOCKER_CERT_PATH-unset}\" >> \"$FAKE_DOCKER_ENV_LOG\"\n\
             if [ -n \"$DX_LEAK_PROBE\" ]; then echo \"$1\" >> \"$FAKE_DOCKER_LEAK_LOG\"; fi\n\
             if [ \"$1 $2\" = \"context inspect\" ]; then\n\
             if [ -z \"$FAKE_DOCKER_CONTEXT_JSON\" ]; then exit 1; fi\n\
             echo \"$FAKE_DOCKER_CONTEXT_JSON\"; exit 0\n\
             fi\n\
             if [ \"$1\" = login ]; then cat > /dev/null; cp \"$DOCKER_CONFIG/config.json\" \"$FAKE_DOCKER_CONFIG_COPY\"; fi\n\
             if [ \"$1\" = \"$FAKE_DOCKER_FAIL\" ]; then exit 1; fi\n\
             if [ \"$1\" = load ] && [ -n \"$FAKE_LOADED\" ]; then echo \"$FAKE_LOADED\"; fi\n\
             if [ \"$1 $2\" = \"image inspect\" ]; then\n\
             case \"$5\" in sha256:*) echo \"$5\" ;; *) echo \"$FAKE_REF_ID\" ;; esac\n\
             fi\n\
             exit 0\n",
        );
        std::fs::write(&script, text).expect("script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    #[cfg(unix)]
    fn run_fake_live_push(
        fail: &str,
        ref_id: Option<&str>,
        loaded: Option<&str>,
        rootfs: bool,
    ) -> (i32, Vec<String>) {
        let scratch = scratch_dir();
        let (archive, id) = oci_save(
            scratch.path(),
            "image.tar",
            &[oci_test_layer(false)],
            "linux/amd64",
        );
        if rootfs {
            std::fs::write(&archive, oci_test_layer(false)).expect("rootfs");
        }
        let classic = format!("Loaded image ID: {id}");
        run_fake_live_push_archive(
            &archive,
            fail,
            ref_id.unwrap_or(&id),
            loaded.unwrap_or(&classic),
            "linux/amd64",
        )
    }

    #[cfg(unix)]
    fn run_fake_live_push_archive(
        archive: &Path,
        fail: &str,
        ref_id: &str,
        loaded: &str,
        platform: &str,
    ) -> (i32, Vec<String>) {
        let run =
            run_fake_live_push_env(archive, fail, ref_id, loaded, platform, &FakeEnv::default());
        (run.code, run.calls)
    }

    /// Caller environment for one fake live push.
    #[cfg(unix)]
    #[derive(Default)]
    struct FakeEnv<'a> {
        docker_host: Option<&'a str>,
        docker_context: Option<&'a str>,
        tls_verify: Option<&'a str>,
        cert_path: Option<&'a str>,
        context_json: &'a str,
    }

    #[cfg(unix)]
    struct FakeRun {
        code: i32,
        /// Docker calls, without the leading `context inspect`.
        calls: Vec<String>,
        inspected: bool,
        /// `verb|DOCKER_CONFIG|DOCKER_HOST|DOCKER_CONTEXT|DOCKER_CERT_PATH` per call.
        env: Vec<String>,
        /// Verbs of the scoped docker calls that saw the caller's DX_LEAK_PROBE.
        leaked: Vec<String>,
        config_copy: Option<String>,
        caller_config: std::path::PathBuf,
    }

    #[cfg(unix)]
    fn run_fake_live_push_env(
        archive: &Path,
        fail: &str,
        ref_id: &str,
        loaded: &str,
        platform: &str,
        caller: &FakeEnv<'_>,
    ) -> FakeRun {
        let scratch = scratch_dir();
        let bin = scratch.path().join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        let log = scratch.path().join("docker.log");
        let env_log = scratch.path().join("docker.env.log");
        let leak_log = scratch.path().join("docker.leak.log");
        let copy = scratch.path().join("config.copy.json");
        let caller_config = scratch.path().join("home-docker");
        fake_docker(
            &bin,
            &[
                ("FAKE_DOCKER_LOG", log.to_string_lossy().as_ref()),
                ("FAKE_DOCKER_ENV_LOG", env_log.to_string_lossy().as_ref()),
                ("FAKE_DOCKER_LEAK_LOG", leak_log.to_string_lossy().as_ref()),
                ("FAKE_DOCKER_CONFIG_COPY", copy.to_string_lossy().as_ref()),
                ("FAKE_DOCKER_FAIL", fail),
                ("FAKE_REF_ID", ref_id),
                ("FAKE_LOADED", loaded),
                ("FAKE_DOCKER_CONTEXT_JSON", caller.context_json),
            ],
        );
        let path = format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let code = with_env(
            &[
                ("PATH", Some(path.as_str())),
                ("DX_LEAK_PROBE", Some("1")),
                (
                    "DOCKER_CONFIG",
                    Some(caller_config.to_string_lossy().as_ref()),
                ),
                ("DOCKER_HOST", caller.docker_host),
                ("DOCKER_CONTEXT", caller.docker_context),
                ("DOCKER_TLS_VERIFY", caller.tls_verify),
                ("DOCKER_CERT_PATH", caller.cert_path),
            ],
            || {
                oci_live_push(
                    archive,
                    "registry.invalid",
                    "demo/app",
                    "1.2.3",
                    platform,
                    "me",
                    "s3cret",
                )
            },
        );
        let read = |path: &Path| -> Vec<String> {
            std::fs::read_to_string(path)
                .unwrap_or_default()
                .lines()
                .map(str::to_owned)
                .collect()
        };
        let mut calls = read(&log);
        let mut env = read(&env_log);
        let leaked: Vec<String> = read(&leak_log)
            .into_iter()
            .filter(|verb| verb != "context")
            .collect();
        let inspected = calls
            .first()
            .is_some_and(|call| call.starts_with("context inspect"));
        if inspected {
            calls.remove(0);
            env.remove(0);
        }
        assert!(calls
            .iter()
            .all(|call| !call.starts_with("context inspect")));
        FakeRun {
            code,
            calls,
            inspected,
            env,
            leaked,
            config_copy: std::fs::read_to_string(&copy).ok(),
            caller_config,
        }
    }

    #[cfg(unix)]
    #[test]
    fn oci_live_push_tags_the_declared_image_before_pushing() {
        let (code, calls) = run_fake_live_push("", None, None, false);
        assert_eq!(code, 0);
        let id = calls[2].split(' ').nth(1).expect("tag source").to_owned();
        assert!(id.starts_with("sha256:"));
        assert_eq!(calls[0], "login registry.invalid -u me --password-stdin");
        assert!(calls[1].starts_with("load -q -i "));
        assert_eq!(
            calls[2],
            format!("tag {id} registry.invalid/demo/app:1.2.3")
        );
        assert_eq!(calls[5], "push registry.invalid/demo/app:1.2.3");
        assert!(calls.iter().all(|call| !call.contains("s3cret")));
    }

    #[cfg(unix)]
    #[test]
    fn oci_live_push_stops_before_push_on_failures() {
        let (code, calls) = run_fake_live_push("", Some("sha256:stale"), None, false);
        assert_eq!(code, 1);
        assert!(calls.iter().all(|call| !call.starts_with("push")));
        for step in ["login", "load", "tag"] {
            let (code, calls) = run_fake_live_push(step, None, None, false);
            assert_eq!(code, 1, "{step}");
            assert!(calls.iter().all(|call| !call.starts_with("push")), "{step}");
        }
        let (code, calls) = run_fake_live_push("", None, Some(""), false);
        assert_eq!(code, 1);
        assert!(calls.iter().any(|call| call.starts_with("load")));
        assert!(calls
            .iter()
            .all(|call| !call.starts_with("tag") && !call.starts_with("push")));
        let manifest = format!("sha256:{}", "ab".repeat(32));
        let containerd = format!("Loaded image ID: {manifest}");
        let (code, calls) = run_fake_live_push("", Some("sha256:stale"), Some(&containerd), false);
        assert_eq!(code, 1);
        assert!(calls.contains(&format!("tag {manifest} registry.invalid/demo/app:1.2.3")));
        assert!(calls.iter().all(|call| !call.starts_with("push")));
        let (code, calls) = run_fake_live_push("", None, None, true);
        assert_eq!(code, 1);
        assert!(calls.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn oci_live_push_tags_what_docker_load_reported() {
        let manifest = format!("sha256:{}", "ab".repeat(32));
        let containerd = format!("Loaded image ID: {manifest}");
        let (code, calls) = run_fake_live_push("", Some(&manifest), Some(&containerd), false);
        assert_eq!(code, 0, "{calls:?}");
        assert_eq!(
            calls[2],
            format!("tag {manifest} registry.invalid/demo/app:1.2.3")
        );
        assert_eq!(calls[5], "push registry.invalid/demo/app:1.2.3");
        let ref_id = format!("sha256:{}", "cd".repeat(32));
        let (code, calls) =
            run_fake_live_push("", Some(&ref_id), Some("Loaded image: demo:1"), false);
        assert_eq!(code, 0, "{calls:?}");
        assert_eq!(calls[2], "tag demo:1 registry.invalid/demo/app:1.2.3");
        assert!(calls.contains(&"push registry.invalid/demo/app:1.2.3".to_owned()));
    }

    #[cfg(unix)]
    fn fake_push(fail: &str, caller: &FakeEnv<'_>) -> FakeRun {
        let scratch = scratch_dir();
        let (archive, id) = oci_save(
            scratch.path(),
            "image.tar",
            &[oci_test_layer(false)],
            "linux/amd64",
        );
        let loaded = format!("Loaded image ID: {id}");
        run_fake_live_push_env(&archive, fail, &id, &loaded, "linux/amd64", caller)
    }

    #[cfg(unix)]
    #[test]
    fn oci_live_push_scopes_docker_config_to_a_temp_dir() {
        for fail in ["", "login", "load", "tag", "push"] {
            let run = fake_push(fail, &FakeEnv::default());
            assert_eq!(run.code, i32::from(!fail.is_empty()), "{fail}");
            assert!(!run.env.is_empty(), "{fail}");
            assert!(run.leaked.is_empty(), "{fail}: {:?}", run.leaked);
            let configs: std::collections::BTreeSet<&str> = run
                .env
                .iter()
                .map(|line| line.split('|').nth(1).expect("config"))
                .collect();
            assert_eq!(configs.len(), 1, "{fail}: {configs:?}");
            let config = Path::new(configs.iter().next().expect("one"));
            assert!(!config.as_os_str().is_empty(), "{fail}");
            assert_ne!(config, run.caller_config, "{fail}");
            assert!(!config.exists(), "{fail}: temp config left behind");
            assert!(!run.caller_config.join("config.json").exists(), "{fail}");
            let copy = run.config_copy.expect("login saw a config");
            let value: serde_json::Value = serde_json::from_str(&copy).expect("json");
            assert!(value["auths"]["registry.invalid"].is_object(), "{fail}");
            assert!(value.get("credsStore").is_none() && value.get("credHelpers").is_none());
            assert!(!copy.contains("s3cret"), "{fail}");
            assert!(
                run.env.iter().all(|line| !line.contains("s3cret")),
                "{fail}"
            );
            assert!(
                run.calls.iter().all(|call| !call.contains("s3cret")),
                "{fail}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn oci_live_push_forwards_the_callers_docker_endpoint() {
        let colima = r#"{"Name":"colima","Endpoints":{"docker":{"Host":"unix:///tmp/colima.sock"}},"TLSMaterial":{}}"#;
        let run = fake_push(
            "",
            &FakeEnv {
                docker_context: Some("colima"),
                context_json: colima,
                ..FakeEnv::default()
            },
        );
        assert_eq!(run.code, 0, "{:?}", run.calls);
        assert!(run.inspected);
        assert_eq!(run.env.len(), 6);
        for line in &run.env {
            let fields: Vec<&str> = line.split('|').collect();
            assert_eq!(fields[2], "unix:///tmp/colima.sock", "{line}");
            assert_eq!(fields[3], "unset", "{line}");
        }

        let run = fake_push(
            "",
            &FakeEnv {
                docker_host: Some("tcp://example.invalid:2375"),
                context_json: colima,
                ..FakeEnv::default()
            },
        );
        assert_eq!(run.code, 0);
        assert!(!run.inspected);
        assert!(run
            .env
            .iter()
            .all(|line| line.split('|').nth(2) == Some("tcp://example.invalid:2375")));

        let run = fake_push("", &FakeEnv::default());
        assert_eq!(run.code, 0);
        assert!(run.inspected);
        assert!(run
            .env
            .iter()
            .all(|line| line.split('|').nth(2) == Some("unset")));

        let tls = r#"{"Endpoints":{"docker":{"Host":"tcp://tls.invalid:2376"}},"TLSMaterial":{"docker":["ca.pem","cert.pem","key.pem"]}}"#;
        let run = fake_push(
            "",
            &FakeEnv {
                context_json: tls,
                ..FakeEnv::default()
            },
        );
        assert_eq!(run.code, 1);
        assert!(run.calls.is_empty() && run.env.is_empty());

        let run = fake_push(
            "",
            &FakeEnv {
                tls_verify: Some("1"),
                ..FakeEnv::default()
            },
        );
        assert_eq!(run.code, 0);
        let expected = run.caller_config.to_string_lossy().into_owned();
        assert!(run
            .env
            .iter()
            .all(|line| line.split('|').nth(4) == Some(expected.as_str())));
    }

    #[test]
    fn private_stage_dir_is_removed_on_drop() {
        let dir = PrivateStageDir::new("dx-private-stage-test-").expect("dir");
        let path = dir.path().to_path_buf();
        write_secure_file(&path, "secret", "x").expect("write");
        assert!(path.join("secret").exists());
        drop(dir);
        assert!(!path.exists());
    }

    #[test]
    fn owner_gates_accept_only_exact_approval() {
        let gates = [
            &PYPI_GATE,
            &NUGET_GATE,
            &CRATES_GATE,
            &GITHUB_GATE,
            &maven::MAVEN_GATE,
            &OCI_GATE,
            &OCTOPUS_GATE,
            &PROMOTION_GATE,
            &NPM_GATE,
        ];
        let tools: std::collections::BTreeSet<&str> = gates.iter().map(|gate| gate.tool).collect();
        let approvals: std::collections::BTreeSet<&str> =
            gates.iter().map(|gate| gate.approved).collect();
        assert_eq!(tools.len(), gates.len());
        assert_eq!(approvals.len(), gates.len());
        for gate in gates {
            let base: std::collections::HashMap<&str, &str> =
                gate.credentials.iter().map(|key| (*key, "x")).collect();
            let run = |env: &std::collections::HashMap<&str, &str>| {
                gate.refusal(|key| env.get(key).copied().unwrap_or_default().to_owned())
            };
            let approval_text = format!("{}=1 (owner approval)", gate.approved);
            let message = run(&base).unwrap_or_default();
            assert!(
                message.starts_with(&format!("{}: ", gate.tool)),
                "{message}"
            );
            assert!(message.contains(&approval_text), "{message}");
            for value in ["", "0", "true", "yes", " 1", "1 "] {
                let mut env = base.clone();
                env.insert(gate.approved, value);
                let message = run(&env).unwrap_or_default();
                assert!(
                    message.starts_with(&format!("{}: ", gate.tool)),
                    "{value:?}: {message}"
                );
                assert!(message.contains(&approval_text), "{value:?}: {message}");
            }
            let mut env = base.clone();
            env.insert(gate.approved, "1");
            assert_eq!(run(&env), None, "{}", gate.tool);
            for credential in gate.credentials {
                let mut env = env.clone();
                env.insert(credential, "");
                let message = run(&env).unwrap_or_default();
                assert!(
                    message.starts_with(&format!("{}: ", gate.tool)),
                    "{message}"
                );
                assert!(message.contains(credential), "{message}");
                assert!(
                    message.contains(&format!("{}=1", gate.approved)),
                    "{message}"
                );
            }
            let lookup = |env: &std::collections::HashMap<&str, &str>, key: &str| {
                env.get(key).copied().unwrap_or_default().to_owned()
            };
            for env in [base.clone(), env.clone()] {
                let refusal = run(&env);
                match gate.authorize(|key| lookup(&env, key)) {
                    Ok(secrets) => {
                        assert_eq!(refusal, None, "{}", gate.tool);
                        for credential in gate.credentials {
                            assert_eq!(secrets.get(credential), "x");
                        }
                    }
                    Err(error) => assert_eq!(Some(error), refusal, "{}", gate.tool),
                }
            }
            for value in ["", "0", "true", "yes", " 1", "1 "] {
                let mut env = base.clone();
                env.insert(gate.live, value);
                assert!(!gate.requested(|key| lookup(&env, key)), "{value:?}");
            }
            let mut env = base.clone();
            env.insert(gate.live, "1");
            assert!(gate.requested(|key| lookup(&env, key)), "{}", gate.tool);
        }
        let lives: std::collections::BTreeSet<&str> = gates.iter().map(|gate| gate.live).collect();
        assert_eq!(lives.len(), gates.len());
    }

    #[test]
    fn launchers_refuse_live_publish_without_owner_approval() {
        if env_test_in_child("tests::launchers_refuse_live_publish_without_owner_approval") {
            return;
        }
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
                    nuget_main(missing, "demo", "", "", "https://nuget.invalid", &argv),
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
                    oci_main(
                        missing,
                        "registry.invalid",
                        "demo/app",
                        "0.0.0",
                        "linux/amd64",
                        &argv
                    ),
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
                assert_eq!(crates_main(missing, "demo", "0.0.0", &argv), 1);
            },
        );
        let crates_runfiles = outdir.join("crates_runfiles");
        stage_runfiles_inputs(&crates_runfiles);
        with_env(
            &[
                (
                    "RUNFILES_DIR",
                    Some(crates_runfiles.to_string_lossy().as_ref()),
                ),
                ("RUNFILES_MANIFEST_FILE", None),
                ("CRATES_PUBLISH_LIVE", Some("1")),
                ("CRATES_ALLOW_DIRTY", Some("1")),
                ("CRATES_PUBLISH_APPROVED", None),
                ("CARGO_REGISTRY_TOKEN", None),
            ],
            || {
                assert_eq!(
                    crates_main("Cargo.toml;lib.rs", "crates_demo", "1.0.0", &argv),
                    1,
                    "CRATES_ALLOW_DIRTY=1 must refuse a live publish"
                );
            },
        );
        with_env(
            &[
                (
                    "RUNFILES_DIR",
                    Some(outdir.join("runfiles").to_string_lossy().as_ref()),
                ),
                ("RUNFILES_MANIFEST_FILE", None),
                ("NPM_PUBLISH_LIVE", Some("1")),
                ("NPM_PUBLISH_APPROVED", Some("1")),
                ("NPM_TOKEN", None),
                ("NPM_PUBLISH_DRY_RUN", None),
            ],
            || {
                assert_eq!(npm_deploy_main(&argv, missing, missing), 1);
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
        if env_test_in_child("tests::archive_and_promotion_launchers_stage_locally") {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        let app = runfiles.join("app_name.txt");
        std::fs::write(&app, b"demo_app\n").expect("write app");
        let tarball = runfiles.join("release_demo.tar.gz");
        let tarball_bytes = archive_bytes(b"payload", "release_demo.tar.gz", false).expect("tar");
        std::fs::write(&tarball, &tarball_bytes).expect("write tarball");
        let digest = dx_digest::sha256_file_hex(&tarball).expect("digest");
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
                        "app_name.txt",
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

    #[cfg(unix)]
    #[test]
    fn promotion_live_records_health_and_refuses() {
        use std::os::unix::fs::PermissionsExt;
        if env_test_in_child("tests::promotion_live_records_health_and_refuses") {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        let tarball = runfiles.join("release_demo.tar.gz");
        std::fs::write(
            &tarball,
            archive_bytes(b"payload", "release_demo.tar.gz", false).expect("tar"),
        )
        .expect("write tarball");
        let bin = scratch.path().join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        let script = bin.join("dx_fake_health");
        std::fs::write(&script, "#!/bin/sh\nexit 0\n").expect("write health");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        for (require, expected) in [(Some("1"), "passed"), (None, "not-required")] {
            with_env(
                &[
                    ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                    ("RUNFILES_MANIFEST_FILE", None),
                    ("BUILD_WORKSPACE_DIRECTORY", None),
                    ("PROMOTION_LIVE", Some("1")),
                    ("PROMOTION_APPROVED", Some("1")),
                    ("PROMOTION_REQUIRE_HEALTH", require),
                    ("PROMOTION_HEALTH_CMD", Some("dx_fake_health")),
                    ("PATH", Some(bin.to_string_lossy().as_ref())),
                    ("PROMOTION_SECRET_REFS", None),
                    ("PROMOTION_REGISTRY_REQUIRED", None),
                    ("PROMOTION_ROLLBACK", None),
                ],
                || {
                    let outdir = tempfile_stage_dir("dx-deploy-promotion-live-").expect("outdir");
                    let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
                    assert_eq!(
                        promotion_main(
                            "release_demo.tar.gz",
                            "promotion_demo",
                            "staging",
                            "production",
                            "1.2.3",
                            &argv
                        ),
                        1
                    );
                    let record: serde_json::Value = serde_json::from_str(
                        &std::fs::read_to_string(
                            outdir.join("promotion_demo-promotion/promotion.json"),
                        )
                        .expect("record"),
                    )
                    .expect("record parses");
                    assert_eq!(record["health"], expected);
                    std::fs::remove_dir_all(&outdir).expect("cleanup");
                },
            );
        }
    }

    fn stage_runfiles_inputs(root: &Path) {
        std::fs::create_dir_all(root).expect("runfiles");
        let maven_pom = crate::maven::tests::central_pom("com.example", "maven_demo", "1.0.0");
        for (name, body) in [
            ("pypi_demo-0.0.0-py3-none-any.whl", &b"wheel"[..]),
            ("pypi_demo-0.0.0.tar.gz", &b"sdist"[..]),
            ("maven_demo-0.0.0.jar", &b"jar"[..]),
            ("maven_demo-0.0.0.pom", maven_pom.as_bytes()),
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
        let layer = root.join("oci_demo_layer.tar");
        std::fs::write(&layer, oci_test_layer(true)).expect("write oci layer");
        archive_image(
            &[layer.as_path()],
            "linux/amd64",
            &["/app/hello"],
            &root.join("oci_demo.tar"),
        )
        .expect("write oci image");
        write_nupkg(
            &root.join("nuget_demo.0.0.0.nupkg"),
            &[("nuget_demo.nuspec", &nuspec("nuget_demo", "1.0.0"))],
        );
    }

    #[test]
    fn launchers_stage_locally_without_publishing() {
        if env_test_in_child("tests::launchers_stage_locally_without_publishing") {
            return;
        }
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
                        "",
                        "1.0.0",
                        "https://nuget.invalid",
                        &argv
                    ),
                    0
                );
                assert_eq!(
                    crates_main("Cargo.toml;lib.rs", "crates_demo", "1.0.0", &argv),
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
                        "linux/amd64",
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
    fn launcher_relative_outdir_resolves_against_invocation_dir() {
        if env_test_in_child("tests::launcher_relative_outdir_resolves_against_invocation_dir") {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        stage_runfiles_inputs(&runfiles);
        let cwd = scratch.path().join("cwd");
        let invocation = scratch.path().join("inv");
        std::fs::create_dir_all(&cwd).expect("cwd");
        std::fs::create_dir_all(&invocation).expect("inv");
        std::env::set_current_dir(&cwd).expect("chdir");
        let argv = vec!["prog".to_owned(), "dist".to_owned()];
        with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                (
                    "BUILD_WORKING_DIRECTORY",
                    Some(invocation.to_string_lossy().as_ref()),
                ),
                ("GH_RELEASE_LIVE", None),
                ("GH_RELEASE_DRY_RUN", Some("1")),
            ],
            || {
                assert_eq!(
                    github_main("release_demo.tar.gz", "demo", "v1.0.0", &argv),
                    0
                );
            },
        );
        assert!(invocation.join("dist").join("demo-release").is_dir());
        assert!(!cwd.join("dist").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let bin = scratch.path().join("bin");
            let workspace = scratch.path().join("ws");
            std::fs::create_dir_all(&bin).expect("bin");
            std::fs::create_dir_all(&workspace).expect("ws");
            let log = scratch.path().join("gh.log");
            let env_log = scratch.path().join("gh.env");
            let script = bin.join("gh");
            std::fs::write(
                &script,
                format!(
                    "#!/bin/sh\npwd -P > '{}'\nprintf '%s\\n' \"$@\" >> '{}'\nenv > '{}'\n",
                    log.display(),
                    log.display(),
                    env_log.display()
                ),
            )
            .expect("script");
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
                .expect("chmod");
            std::env::set_current_dir(scratch.path()).expect("chdir");
            let path = format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            );
            with_env(
                &[
                    ("PATH", Some(path.as_str())),
                    ("RUNFILES_DIR", Some("runfiles")),
                    ("RUNFILES_MANIFEST_FILE", None),
                    (
                        "BUILD_WORKING_DIRECTORY",
                        Some(invocation.to_string_lossy().as_ref()),
                    ),
                    (
                        "BUILD_WORKSPACE_DIRECTORY",
                        Some(workspace.to_string_lossy().as_ref()),
                    ),
                    ("GH_REPO", None),
                    ("GH_RELEASE_LIVE", Some("1")),
                    ("GH_RELEASE_APPROVED", Some("1")),
                    ("GH_TOKEN", Some("gh-sekret")),
                    ("DX_LEAK_PROBE", Some("1")),
                ],
                || {
                    assert_eq!(
                        github_main("release_demo.tar.gz", "demo", "v1.0.0", &argv),
                        0
                    );
                },
            );
            let logged = std::fs::read_to_string(&log).expect("gh log");
            let lines: Vec<&str> = logged.lines().collect();
            let workspace_real = std::fs::canonicalize(&workspace).expect("canonical ws");
            assert_eq!(lines[0], workspace_real.to_string_lossy(), "{logged}");
            assert_eq!(&lines[1..4], ["release", "create", "v1.0.0"], "{logged}");
            let asset = Path::new(lines[4]);
            assert!(asset.is_absolute(), "{logged}");
            assert_eq!(
                std::fs::canonicalize(asset).expect("canonical asset"),
                std::fs::canonicalize(runfiles.join("release_demo.tar.gz")).expect("canonical"),
                "{logged}"
            );
            assert_eq!(&lines[5..], ["--draft", "--verify-tag"], "{logged}");
            let env = std::fs::read_to_string(&env_log).expect("gh env");
            assert!(env.lines().any(|line| line == "GH_TOKEN=gh-sekret"));
            assert!(!env.contains("DX_LEAK_PROBE"), "{env}");
            assert!(!logged.contains("gh-sekret"), "{logged}");
        }
    }

    #[test]
    fn build_helpers_reject_unusable_inputs() {
        let scratch = scratch_dir();
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let image = scratch.path().join("oci_demo.bin");
        std::fs::write(&image, b"image").expect("write image");
        let error = oci_build_layout(
            &image,
            &outdir,
            "registry.invalid",
            "demo/app",
            "1.0.0",
            "linux/amd64",
        )
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
    fn oci_layout_rejects_unusable_registry_repository_and_tag() {
        let scratch = scratch_dir();
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let (image, _) = oci_save(
            scratch.path(),
            "oci_demo.tar",
            &[oci_test_layer(false)],
            "linux/amd64",
        );
        for registry in ["", "https://registry.invalid"] {
            let error = oci_build_layout(
                &image,
                &outdir,
                registry,
                "demo/app",
                "1.0.0",
                "linux/amd64",
            )
            .expect_err("registry host");
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{registry}");
        }
        for repository in ["", "/demo/app", "demo/app/"] {
            let error = oci_build_layout(
                &image,
                &outdir,
                "registry.invalid",
                repository,
                "1.0.0",
                "linux/amd64",
            )
            .expect_err("repository path");
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{repository}");
        }
        let error = oci_build_layout(
            &image,
            &outdir,
            "registry.invalid",
            "demo/app",
            "",
            "linux/amd64",
        )
        .expect_err("empty tag");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(std::fs::read_dir(&outdir).expect("outdir").next().is_none());
    }

    fn oci_layout_json(layout: &Path) -> (serde_json::Value, serde_json::Value) {
        let blobs = layout.join("blobs").join("sha256");
        let read = |digest: &str| {
            let body = std::fs::read_to_string(blobs.join(&digest[7..])).expect("blob");
            serde_json::from_str::<serde_json::Value>(&body).expect("json")
        };
        let index = std::fs::read_to_string(layout.join("index.json")).expect("index");
        let index: serde_json::Value = serde_json::from_str(&index).expect("index parses");
        let manifest = read(index["manifests"][0]["digest"].as_str().expect("digest"));
        let config = read(manifest["config"]["digest"].as_str().expect("config"));
        (manifest, config)
    }

    #[test]
    fn oci_layout_diff_id_is_the_uncompressed_digest() {
        let scratch = scratch_dir();
        let tar = oci_test_layer(false);
        let gz = oci_test_layer(true);
        let tar_digest = format!("sha256:{}", dx_digest::sha256_hex(&tar));
        let gz_digest = format!("sha256:{}", dx_digest::sha256_hex(&gz));
        assert_ne!(tar_digest, gz_digest);
        for (name, bytes, blob_digest) in [
            ("plain.tar", &tar, &tar_digest),
            ("gzip.tar", &gz, &gz_digest),
        ] {
            let (image, _) = oci_save(
                scratch.path(),
                name,
                std::slice::from_ref(bytes),
                "linux/amd64",
            );
            let outdir = scratch.path().join(name.replace('.', "_"));
            std::fs::create_dir_all(&outdir).expect("outdir");
            let layout = oci_build_layout(
                &image,
                &outdir,
                "registry.invalid",
                "demo/app",
                "1.0.0",
                "linux/amd64",
            )
            .expect("layout");
            let (manifest, config) = oci_layout_json(&layout);
            assert_eq!(
                manifest["layers"][0]["digest"],
                blob_digest.as_str(),
                "{name}"
            );
            assert_eq!(manifest["layers"][0]["size"], bytes.len(), "{name}");
            assert_eq!(
                config["rootfs"]["diff_ids"][0],
                tar_digest.as_str(),
                "{name}"
            );
            let stored = layout.join("blobs").join("sha256").join(&blob_digest[7..]);
            assert_eq!(std::fs::read(stored).expect("blob"), *bytes, "{name}");
        }
    }

    #[test]
    fn oci_layout_rejects_malformed_gzip_layers() {
        let scratch = scratch_dir();
        let gz = oci_test_layer(true);
        let truncated = gz[..gz.len() - 12].to_vec();
        let mut corrupt = gz.clone();
        let mid = corrupt.len() / 2;
        corrupt[mid] ^= 0xff;
        let tar = oci_test_layer(false);
        let config = oci_config("amd64", &[&tar]);
        let mut cases: Vec<(String, std::path::PathBuf)> = Vec::new();
        for (name, bytes) in [
            ("not_gzip", b"not gzip at all".to_vec()),
            ("truncated", truncated),
            ("corrupt", corrupt),
        ] {
            let image = scratch.path().join(format!("{name}.tar"));
            oci_tar(
                &image,
                &[
                    (
                        "manifest.json",
                        b"[{\"Config\":\"c.json\",\"Layers\":[\"l/layer.tar\"]}]",
                    ),
                    ("c.json", &config),
                    ("l/layer.tar", &bytes),
                ],
                &[],
            );
            cases.push((name.to_owned(), image));
        }
        let (valid, _) = oci_save(scratch.path(), "valid.tar", &[tar.clone()], "linux/amd64");
        let outer = gzip_compress(&std::fs::read(&valid).expect("read valid")).expect("gzip");
        let mut flipped = outer.clone();
        let mid = flipped.len() / 2;
        flipped[mid] ^= 0xff;
        for (name, bytes) in [
            ("outer_truncated", outer[..outer.len() / 2].to_vec()),
            ("outer_corrupt", flipped),
        ] {
            let image = scratch.path().join(format!("{name}.tar.gz"));
            std::fs::write(&image, bytes).expect("write image");
            cases.push((name.to_owned(), image));
        }
        for (name, image) in cases {
            let outdir = scratch.path().join(&name);
            std::fs::create_dir_all(&outdir).expect("outdir");
            let error = oci_build_layout(
                &image,
                &outdir,
                "registry.invalid",
                "demo/app",
                "1.0.0",
                "linux/amd64",
            )
            .expect_err(&name);
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{name}");
            assert!(
                oci_declared_image_id(&image, "linux/amd64").is_err(),
                "{name}"
            );
            assert!(
                std::fs::read_dir(&outdir).expect("outdir").next().is_none(),
                "{name}"
            );
        }
    }

    #[test]
    fn oci_paths_reject_zstd_compression() {
        let scratch = scratch_dir();
        let mut zstd = ZSTD_MAGIC.to_vec();
        zstd.extend_from_slice(b"not a real zstd frame");
        let tar = oci_test_layer(false);
        let config = oci_config("amd64", &[&tar]);
        let layer = scratch.path().join("zstd_layer.tar");
        oci_tar(
            &layer,
            &[
                (
                    "manifest.json",
                    b"[{\"Config\":\"c.json\",\"Layers\":[\"l/layer.tar\"]}]",
                ),
                ("c.json", &config),
                ("l/layer.tar", &zstd),
            ],
            &[],
        );
        let (valid, _) = oci_save(scratch.path(), "valid.tar", &[tar.clone()], "linux/amd64");
        let mut outer = ZSTD_MAGIC.to_vec();
        outer.extend_from_slice(&std::fs::read(&valid).expect("read valid"));
        let archive = scratch.path().join("zstd_image.tar");
        std::fs::write(&archive, outer).expect("write image");
        for (name, image, want) in [
            ("layer", layer, "unsupported layer compression zstd"),
            (
                "archive",
                archive,
                "unsupported image archive compression zstd",
            ),
        ] {
            let outdir = scratch.path().join(name);
            std::fs::create_dir_all(&outdir).expect("outdir");
            let error = oci_build_layout(
                &image,
                &outdir,
                "registry.invalid",
                "demo/app",
                "1.0.0",
                "linux/amd64",
            )
            .expect_err(name);
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{name}");
            let message = error.to_string();
            assert!(message.contains(want), "{name}: {message}");
            assert!(
                !message.contains("diff_ids do not match"),
                "{name}: {message}"
            );
            let id_error = oci_declared_image_id(&image, "linux/amd64").expect_err(name);
            assert!(id_error.contains(want), "{name}: {id_error}");
            assert!(
                std::fs::read_dir(&outdir).expect("outdir").next().is_none(),
                "{name}"
            );
        }
    }

    fn elf_header(machine: u16) -> Vec<u8> {
        let mut header = vec![0u8; 64];
        header[..4].copy_from_slice(b"\x7fELF");
        header[4] = 2;
        header[5] = 1;
        header[6] = 1;
        header[16..18].copy_from_slice(&2u16.to_le_bytes());
        header[18..20].copy_from_slice(&machine.to_le_bytes());
        header
    }

    #[test]
    fn oci_layout_platform_matches_the_artifact_bytes() {
        let scratch = scratch_dir();
        for (platform, machine, architecture) in [
            ("linux/amd64", 0x3eu16, "amd64"),
            ("linux/arm64", 0xb7u16, "arm64"),
        ] {
            let elf = elf_header(machine);
            let tar = tar_archive([("app", elf.as_slice(), 0o755)].into_iter()).expect("tar");
            let name = format!("{architecture}.tar");
            let (image, _) = oci_save(scratch.path(), &name, &[tar], platform);
            let outdir = scratch.path().join(architecture);
            std::fs::create_dir_all(&outdir).expect("outdir");
            let layout = oci_build_layout(
                &image,
                &outdir,
                "registry.invalid",
                "demo/app",
                "1.0.0",
                platform,
            )
            .expect("layout");
            let (manifest, config) = oci_layout_json(&layout);
            let blob = manifest["layers"][0]["digest"].as_str().expect("digest");
            let stored = std::fs::read(layout.join("blobs").join("sha256").join(&blob[7..]))
                .expect("layer blob");
            let mut archive = tar::Archive::new(stored.as_slice());
            let mut entry = archive
                .entries()
                .expect("entries")
                .next()
                .expect("entry")
                .expect("entry");
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).expect("read app");
            let observed = match u16::from_le_bytes([bytes[18], bytes[19]]) {
                0x3e => "amd64",
                0xb7 => "arm64",
                other => panic!("unexpected e_machine {other:#x}"),
            };
            assert_eq!(config["architecture"], observed, "{platform}");
            assert_eq!(config["architecture"], architecture, "{platform}");
            assert_eq!(config["os"], "linux", "{platform}");
        }
        let (arm, _) = oci_save(
            scratch.path(),
            "mismatch.tar",
            &[oci_test_layer(false)],
            "linux/arm64",
        );
        let outdir = scratch.path().join("mismatch");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let error = oci_build_layout(
            &arm,
            &outdir,
            "registry.invalid",
            "demo/app",
            "1.0.0",
            "linux/amd64",
        )
        .expect_err("arm64 config staged as amd64");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("linux/arm64"), "{error}");
        assert!(std::fs::read_dir(&outdir).expect("outdir").next().is_none());
    }

    #[test]
    fn oci_layout_rejects_missing_and_unsupported_platforms() {
        let scratch = scratch_dir();
        let (image, _) = oci_save(
            scratch.path(),
            "oci_demo.tar",
            &[oci_test_layer(false)],
            "linux/amd64",
        );
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        for platform in [
            "",
            "windows/amd64",
            "linux/riscv64",
            "amd64",
            "linux/arm64/v8",
        ] {
            let error = oci_build_layout(
                &image,
                &outdir,
                "registry.invalid",
                "demo/app",
                "1.0.0",
                platform,
            )
            .expect_err("unsupported platform");
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{platform}");
            assert!(error.to_string().contains("platform"), "{platform}");
        }
        assert!(std::fs::read_dir(&outdir).expect("outdir").next().is_none());
    }

    #[test]
    fn oci_layout_media_type_follows_the_layer_bytes() {
        let scratch = scratch_dir();
        for name in ["oci_demo.tar", "oci_demo.tar.gz"] {
            let (image, _) = oci_save(
                scratch.path(),
                name,
                &[oci_test_layer(false), oci_test_layer(true)],
                "linux/amd64",
            );
            let outdir = scratch.path().join(name.replace('.', "_"));
            std::fs::create_dir_all(&outdir).expect("outdir");
            let layout = oci_build_layout(
                &image,
                &outdir,
                "registry.invalid",
                "demo/app",
                "1.0.0",
                "linux/amd64",
            )
            .expect("layout");
            let (manifest, _) = oci_layout_json(&layout);
            assert_eq!(
                manifest["layers"][0]["mediaType"], "application/vnd.oci.image.layer.v1.tar",
                "{name}"
            );
            assert_eq!(
                manifest["layers"][1]["mediaType"], "application/vnd.oci.image.layer.v1.tar+gzip",
                "{name}"
            );
        }
    }

    #[test]
    fn oci_layout_keeps_every_docker_save_layer_and_the_config() {
        let scratch = scratch_dir();
        let plain = tar_archive([("etc/demo.conf", b"demo=1\n".as_slice(), 0o644)].into_iter())
            .expect("tar");
        let gz_tar =
            tar_archive([("app/hello", b"hello\n".as_slice(), 0o755)].into_iter()).expect("tar");
        let gz = gzip_compress(&gz_tar).expect("gzip");
        let config = oci_config("amd64", &[&plain, &gz_tar]);
        let config_hex = dx_digest::sha256_hex(&config);
        let plain_hex = dx_digest::sha256_hex(&plain);
        let gz_hex = dx_digest::sha256_hex(&gz);
        let manifest = format!(
            "[{{\"Config\":\"blobs/sha256/{config_hex}\",\"RepoTags\":[\"demo:1\"],\"Layers\":[\"blobs/sha256/{plain_hex}\",\"blobs/sha256/{gz_hex}\"]}}]"
        );
        let image = scratch.path().join("image.tar");
        let config_name = format!("blobs/sha256/{config_hex}");
        let plain_name = format!("blobs/sha256/{plain_hex}");
        let gz_name = format!("blobs/sha256/{gz_hex}");
        oci_tar(
            &image,
            &[
                (gz_name.as_str(), &gz),
                ("manifest.json", manifest.as_bytes()),
                (config_name.as_str(), &config),
                (plain_name.as_str(), &plain),
            ],
            &[],
        );
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let layout = oci_build_layout(
            &image,
            &outdir,
            "registry.invalid",
            "demo/app",
            "1.0.0",
            "linux/amd64",
        )
        .expect("layout");
        let blobs = layout.join("blobs").join("sha256");
        let (manifest, parsed_config) = oci_layout_json(&layout);
        let config_digest = manifest["config"]["digest"]
            .as_str()
            .expect("config digest");
        assert_eq!(config_digest, format!("sha256:{config_hex}"));
        assert_eq!(
            oci_declared_image_id(&image, "linux/amd64").expect("id"),
            config_digest
        );
        assert_eq!(manifest["config"]["size"], config.len());
        assert_eq!(
            std::fs::read(blobs.join(&config_hex)).expect("config blob"),
            config
        );
        assert_eq!(
            parsed_config["config"]["Env"],
            serde_json::json!(["PATH=/usr/bin", "DEMO=1"])
        );
        assert_eq!(
            parsed_config["config"]["Entrypoint"],
            serde_json::json!(["/app/hello"])
        );
        let layers = manifest["layers"].as_array().expect("layers");
        assert_eq!(layers.len(), 2);
        for (layer, (hex, bytes)) in layers.iter().zip([(&plain_hex, &plain), (&gz_hex, &gz)]) {
            assert_eq!(layer["digest"], format!("sha256:{hex}"));
            assert_eq!(layer["size"], bytes.len());
            assert_eq!(std::fs::read(blobs.join(hex)).expect("layer blob"), *bytes);
        }
        let mut stored: Vec<String> = std::fs::read_dir(&blobs)
            .expect("blobs")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        stored.sort();
        assert_eq!(stored.len(), 4, "{stored:?}");
    }

    #[cfg(unix)]
    #[test]
    fn oci_layout_and_live_push_bind_the_same_config_digest() {
        let scratch = scratch_dir();
        let (image, id) = oci_save(
            scratch.path(),
            "image.tar.gz",
            &[oci_test_layer(false), oci_test_layer(true)],
            "linux/amd64",
        );
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let layout = oci_build_layout(
            &image,
            &outdir,
            "registry.invalid",
            "demo/app",
            "1.2.3",
            "linux/amd64",
        )
        .expect("layout");
        let (manifest, _) = oci_layout_json(&layout);
        let config_digest = manifest["config"]["digest"].as_str().expect("digest");
        assert_eq!(config_digest, id);
        let loaded = format!("Loaded image ID: {id}");
        let (code, calls) = run_fake_live_push_archive(&image, "", &id, &loaded, "linux/amd64");
        assert_eq!(code, 0, "{calls:?}");
        assert_eq!(
            calls[2],
            format!("tag {config_digest} registry.invalid/demo/app:1.2.3")
        );
    }

    #[cfg(unix)]
    #[test]
    fn oci_live_push_rejects_what_the_layout_rejects() {
        let scratch = scratch_dir();
        let layers = [oci_test_layer(false)];
        let (arm, arm_id) = oci_save(scratch.path(), "arm.tar", &layers, "linux/arm64");
        let (tgz, tgz_id) = oci_save(scratch.path(), "image.tgz", &layers, "linux/amd64");
        let (amd, amd_id) = oci_save(scratch.path(), "amd.tar", &layers, "linux/amd64");
        let cases = [
            ("arm64 image", &arm, &arm_id, "linux/amd64"),
            ("tgz suffix", &tgz, &tgz_id, "linux/amd64"),
            ("s390x platform", &amd, &amd_id, "linux/s390x"),
        ];
        for (name, image, id, platform) in cases {
            let outdir = scratch.path().join(name.replace(' ', "_"));
            std::fs::create_dir_all(&outdir).expect("outdir");
            let layout = oci_build_layout(
                image,
                &outdir,
                "registry.invalid",
                "demo/app",
                "1.2.3",
                platform,
            )
            .expect_err(name);
            assert_eq!(
                layout.kind(),
                io::ErrorKind::InvalidInput,
                "{name}: {layout}"
            );
            assert_eq!(
                std::fs::read_dir(&outdir).expect("read outdir").count(),
                0,
                "{name}"
            );
            let declared = oci_declared_image_id(image, platform).expect_err(name);
            if name == "arm64 image" {
                assert!(layout.to_string().contains("linux/arm64"), "{layout}");
                assert!(declared.contains("linux/arm64"), "{declared}");
            }
            let loaded = format!("Loaded image ID: {id}");
            let (code, calls) = run_fake_live_push_archive(image, "", id, &loaded, platform);
            assert_eq!(code, 1, "{name}");
            assert!(calls.is_empty(), "{name}: {calls:?}");
        }
        let loaded = format!("Loaded image ID: {arm_id}");
        let (code, calls) = run_fake_live_push_archive(&arm, "", &arm_id, &loaded, "linux/arm64");
        assert_eq!(code, 0, "{calls:?}");
        assert_eq!(
            calls[2],
            format!("tag {arm_id} registry.invalid/demo/app:1.2.3")
        );
    }

    #[test]
    fn oci_paths_reject_rootfs_and_inconsistent_archives() {
        let scratch = scratch_dir();
        let layer = oci_test_layer(false);
        let other =
            tar_archive([("other", b"other\n".as_slice(), 0o644)].into_iter()).expect("tar");
        let config = oci_config("amd64", &[&layer]);
        let twice = oci_config("amd64", &[&layer, &layer]);
        let both = |name: &str| {
            let image = scratch.path().join(name);
            let outdir = scratch.path().join(name.replace('.', "_"));
            std::fs::create_dir_all(&outdir).expect("outdir");
            let id = oci_declared_image_id(&image, "linux/amd64");
            let layout = oci_build_layout(
                &image,
                &outdir,
                "registry.invalid",
                "demo/app",
                "1.0.0",
                "linux/amd64",
            );
            (id, layout, outdir)
        };
        let manifest_one = b"[{\"Config\":\"c.json\",\"Layers\":[\"a/layer.tar\"]}]";
        let manifest_two =
            b"[{\"Config\":\"c.json\",\"Layers\":[\"a/layer.tar\",\"b/layer.tar\"]}]";
        std::fs::write(scratch.path().join("rootfs.tar"), &layer).expect("rootfs");
        oci_tar(
            &scratch.path().join("missing.tar"),
            &[("manifest.json", manifest_one), ("c.json", &config)],
            &[],
        );
        oci_tar(
            &scratch.path().join("mismatch.tar"),
            &[
                ("manifest.json", manifest_one),
                ("c.json", &config),
                ("a/layer.tar", &other),
            ],
            &[],
        );
        oci_tar(
            &scratch.path().join("escape.tar"),
            &[("manifest.json", manifest_one), ("c.json", &config)],
            &[("a/layer.tar", "../../etc/passwd")],
        );
        for (name, want) in [
            ("rootfs.tar", "no manifest.json"),
            ("missing.tar", "missing its layer 'a/layer.tar'"),
            ("mismatch.tar", "diff_ids do not match"),
            ("escape.tar", "escapes the archive"),
        ] {
            let (id, layout, outdir) = both(name);
            let id = id.expect_err(name);
            assert!(id.contains(want), "{name}: {id}");
            let error = layout.expect_err(name);
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{name}");
            assert!(error.to_string().contains(want), "{name}: {error}");
            assert!(
                std::fs::read_dir(&outdir).expect("outdir").next().is_none(),
                "{name}"
            );
        }
        let id_name = dx_digest::sha256_hex(&config);
        let legacy_manifest =
            format!("[{{\"Config\":\"{id_name}.json\",\"Layers\":[\"{id_name}/layer.tar\"]}}]");
        let legacy_config = format!("{id_name}.json");
        let legacy_layer = format!("{id_name}/layer.tar");
        oci_tar(
            &scratch.path().join("legacy.tar"),
            &[
                ("manifest.json", legacy_manifest.as_bytes()),
                (legacy_config.as_str(), &config),
                (legacy_layer.as_str(), &layer),
            ],
            &[],
        );
        let blob_manifest =
            format!("[{{\"Config\":\"blobs/sha256/{id_name}\",\"Layers\":[\"./a/layer.tar\"]}}]");
        let blob_config = format!("blobs/sha256/{id_name}");
        oci_tar(
            &scratch.path().join("blobs.tar"),
            &[
                ("manifest.json", blob_manifest.as_bytes()),
                (blob_config.as_str(), &config),
                ("a/layer.tar", &layer),
            ],
            &[],
        );
        oci_tar(
            &scratch.path().join("linked.tar"),
            &[
                ("manifest.json", manifest_two),
                ("c.json", &twice),
                ("a/layer.tar", &layer),
            ],
            &[("b/layer.tar", "../a/layer.tar")],
        );
        for (name, body) in [
            ("legacy.tar", &config),
            ("blobs.tar", &config),
            ("linked.tar", &twice),
        ] {
            let want = format!("sha256:{}", dx_digest::sha256_hex(body));
            let (id, layout, _) = both(name);
            assert_eq!(id.expect(name), want, "{name}");
            let (manifest, _) = oci_layout_json(&layout.expect(name));
            assert_eq!(manifest["config"]["digest"], want.as_str(), "{name}");
        }
    }

    #[test]
    fn octopus_drop_rejects_empty_spec_fields() {
        let scratch = scratch_dir();
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let package = scratch.path().join("release_demo.tar.gz");
        std::fs::write(&package, b"package").expect("write package");
        let base = OctopusDrop {
            deploy_name: "octopus_demo",
            project: "octopus_demo",
            channel: "Default",
            version: "1.0.0",
            deploy_to: &[],
            space: "",
            server: "https://octopus.example.invalid",
        };
        let empties = [
            (
                "deploy_name",
                OctopusDrop {
                    deploy_name: "",
                    ..base
                },
            ),
            (
                "project",
                OctopusDrop {
                    project: "",
                    ..base
                },
            ),
            (
                "channel",
                OctopusDrop {
                    channel: "",
                    ..base
                },
            ),
            (
                "version",
                OctopusDrop {
                    version: "",
                    ..base
                },
            ),
        ];
        for (field, spec) in empties {
            let error = octopus_build_drop(&package, &outdir, &spec).expect_err("empty spec field");
            assert_eq!(
                error.kind(),
                io::ErrorKind::InvalidInput,
                "{field}: {error}"
            );
        }
    }

    #[test]
    fn promotion_rejects_unusable_environments_and_names() {
        let scratch = scratch_dir();
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let artifact = scratch.path().join("release_demo.tar.gz");
        std::fs::write(&artifact, b"promotion-bytes-v1").expect("write artifact");
        let build = |deploy, from, to, version, src: &Path| {
            promotion_build(
                src,
                &outdir,
                deploy,
                from,
                to,
                version,
                &[],
                PromotionHealth::SkippedLocal,
            )
        };
        let baseline = build(
            "promotion_demo",
            "staging",
            "production",
            "1.2.3",
            &artifact,
        )
        .expect("baseline promotion");
        assert!(baseline.join("promotion.json").is_file());
        let absent = scratch.path().join("absent.tar.gz");
        let error = build("promotion_demo", "staging", "production", "1.2.3", &absent)
            .expect_err("absent artifact");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        let rejects = [
            (
                "deploy",
                build("", "staging", "production", "1.2.3", &artifact),
            ),
            (
                "from",
                build("promotion_demo", "", "production", "1.2.3", &artifact),
            ),
            (
                "to",
                build("promotion_demo", "staging", "", "1.2.3", &artifact),
            ),
            (
                "same",
                build("promotion_demo", "prod", "prod", "1.2.3", &artifact),
            ),
            (
                "version",
                build("promotion_demo", "staging", "production", "", &artifact),
            ),
        ];
        for (field, result) in rejects {
            let error = result.expect_err("unusable promotion field");
            assert_eq!(
                error.kind(),
                io::ErrorKind::InvalidInput,
                "{field}: {error}"
            );
        }
    }

    #[test]
    fn npm_bracketed_filename_round_trips_through_verify() {
        let scratch = scratch_dir();
        let src = scratch.path().join("[id].js");
        std::fs::write(&src, b"module.exports = 1;\n").expect("write src");
        let manifest = scratch.path().join("package.json");
        std::fs::write(
            &manifest,
            b"{\"name\": \"npm-demo\", \"version\": \"1.0.0\"}\n",
        )
        .expect("write manifest");
        let tgz = scratch.path().join("npm_demo.tgz");
        let feed = scratch.path().join("npm_demo.feed.json");
        npm_create_pack(
            "npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "public",
            "required",
            &tgz,
            &feed,
            &[manifest, src],
        )
        .expect("pack");
        let (name, _, _, _, _) = npm_verify_pack(&tgz, &feed).expect("verify");
        assert_eq!(name, "npm-demo");
        let members = npm_tar_members(&tgz).expect("members");
        assert_eq!(members, ["package/[id].js", "package/package.json"]);

        let body = std::fs::read_to_string(&feed).expect("read feed");
        let reordered = scratch.path().join("reordered.feed.json");
        let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
        std::fs::write(&reordered, serde_json::to_string(&parsed).expect("compact"))
            .expect("write");
        npm_verify_pack(&tgz, &reordered).expect("whitespace does not matter");

        let tampered = scratch.path().join("tampered.feed.json");
        std::fs::write(&tampered, body.replace("package/[id].js", "package/[x].js"))
            .expect("write");
        let error = npm_verify_pack(&tgz, &tampered).expect_err("member mismatch");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
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
            "public",
            "required",
            &tgz,
            &feed,
            std::slice::from_ref(&src),
        )
        .expect("pack");
        let (name, tag, registry, _, _) = npm_verify_pack(&tgz, &feed).expect("verify");
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
            "public".to_owned(),
            "required".to_owned(),
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

    fn stage_npm_demo(root: &Path, provenance: &str) -> std::path::PathBuf {
        let runfiles = root.join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        let src = runfiles.join("package.json");
        std::fs::write(&src, b"{\"name\": \"npm-demo\", \"version\": \"1.0.0\"}\n")
            .expect("write src");
        let bracketed = runfiles.join("[id].js");
        std::fs::write(&bracketed, b"module.exports = 1;\n").expect("write bracketed");
        npm_create_pack(
            "npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "public",
            provenance,
            &runfiles.join("npm_demo.tgz"),
            &runfiles.join("npm_demo.feed.json"),
            &[src, bracketed],
        )
        .expect("pack");
        runfiles
    }

    #[test]
    fn npm_deploy_main_releases_a_staged_pack() {
        if env_test_in_child("tests::npm_deploy_main_releases_a_staged_pack") {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = stage_npm_demo(scratch.path(), "required");
        let outdir = tempfile_stage_dir("dx-deploy-npm-").expect("outdir");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                ("DX_PROFILE", Some("dx_dev")),
                ("BUILD_WORKSPACE_DIRECTORY", None),
                ("NPM_PUBLISH_LIVE", None),
                ("NPM_PUBLISH_APPROVED", None),
            ],
            || {
                assert_eq!(
                    npm_deploy_main(&argv, "npm_demo.feed.json", "npm_demo.tgz"),
                    0
                );
            },
        );
        assert!(outdir.join("npm_demo.tgz").is_file());
        assert!(outdir
            .join("npm_demo-feed")
            .join("npm_demo.feed.json")
            .is_file());
        assert!(npm_tar_members(&outdir.join("npm_demo.tgz"))
            .expect("members")
            .contains(&"package/[id].js".to_owned()));
        std::fs::remove_dir_all(&outdir).expect("cleanup");
    }

    #[test]
    fn npm_provenance_policy_rejects_unsupported_combinations() {
        let npmjs = "https://registry.npmjs.org";
        assert!(npm_provenance_error("public", npmjs, "required").is_none());
        assert!(npm_provenance_error("public", npmjs, "none").is_none());
        assert!(npm_provenance_error("restricted", "https://npm.example.com", "none").is_none());
        let restricted = npm_provenance_error("restricted", npmjs, "required").expect("restricted");
        assert!(restricted.contains("not supported with access 'restricted'"));
        let private = npm_provenance_error("public", "https://npm.example.com/repo", "required")
            .expect("registry");
        assert!(private.contains("not supported by registry"));
        let unknown = npm_provenance_error("public", npmjs, "auto").expect("unknown");
        assert!(unknown.contains("want 'required' or 'none'"));
    }

    #[test]
    fn npm_create_pack_refuses_unsupported_provenance_before_writing() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(&src, b"{\"name\": \"x\", \"version\": \"1.0.0\"}\n").expect("write");
        let tgz = scratch.path().join("x.tgz");
        let feed = scratch.path().join("x.feed.json");
        let error = npm_create_pack(
            "@s/x",
            "latest",
            "https://registry.npmjs.org",
            "restricted",
            "required",
            &tgz,
            &feed,
            &[src],
        )
        .expect_err("restricted provenance");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(!tgz.exists() && !feed.exists());
    }

    #[test]
    fn npm_publish_args_state_the_provenance_policy() {
        let tgz = Path::new("x.tgz");
        let required = npm_publish_args(tgz, "latest", "public", "https://r", "required");
        assert!(required.iter().any(|arg| arg == "--provenance"));
        let none = npm_publish_args(tgz, "latest", "restricted", "https://r", "none");
        assert!(none.iter().any(|arg| arg == "--provenance=false"));
        assert!(!none.iter().any(|arg| arg == "--provenance"));
    }

    #[test]
    fn npm_provenance_unavailable_without_identity() {
        let none = [
            ("ACTIONS_ID_TOKEN_REQUEST_URL", None),
            ("SIGSTORE_ID_TOKEN", None),
        ];
        with_env(&none, || {
            let error = npm_provenance_unavailable("required").expect("unavailable");
            assert!(error.contains("publishing nothing"));
            assert!(npm_provenance_unavailable("none").is_none());
        });
        with_env(
            &[
                (
                    "ACTIONS_ID_TOKEN_REQUEST_URL",
                    Some("https://ci.example/token"),
                ),
                ("SIGSTORE_ID_TOKEN", None),
            ],
            || assert!(npm_provenance_unavailable("required").is_none()),
        );
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
    fn fake_octo(dir: &Path, fail_on: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join("octo");
        let d = dir.display();
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" '---' >> '{d}/args.txt'\nenv >> '{d}/env.txt'\nif [ \"$1\" = '{fail_on}' ]; then exit 1; fi\nexit 0\n"
        );
        std::fs::write(&path, script).expect("write octo");
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

    #[cfg(unix)]
    fn octo_spec<'a>(envs: &'a [String], space: &'a str) -> OctopusDrop<'a> {
        OctopusDrop {
            deploy_name: "p",
            project: "p",
            channel: "Default",
            version: "1.0.0",
            deploy_to: envs,
            space,
            server: "https://o.invalid",
        }
    }

    #[cfg(unix)]
    fn octo_calls(dir: &Path) -> Vec<Vec<String>> {
        let text = std::fs::read_to_string(dir.join("args.txt")).unwrap_or_default();
        let mut calls = vec![];
        let mut cur = vec![];
        for line in text.lines() {
            if line == "---" {
                calls.push(std::mem::take(&mut cur));
            } else {
                cur.push(line.to_owned());
            }
        }
        calls
    }

    #[cfg(unix)]
    fn strings(args: Vec<OsString>) -> Vec<String> {
        args.iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[cfg(unix)]
    #[test]
    fn octopus_live_publish_runs_push_then_release() {
        let scratch = scratch_dir();
        let octo = fake_octo(scratch.path(), "none");
        let package = Path::new("/stage/p-1.0.0.tar.gz");
        let envs = ["Production".to_owned()];
        with_env(
            &[
                ("DX_LEAK_PROBE", Some("1")),
                ("OCTOPUS_API_KEY", Some("leaky")),
            ],
            || {
                assert_eq!(
                    octopus_live_publish(&octo, package, &octo_spec(&envs, "S"), "sekret"),
                    0
                );
            },
        );
        let calls = octo_calls(scratch.path());
        let spec = octo_spec(&envs, "S");
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], strings(octopus_push_args(package, &spec)));
        assert_eq!(calls[1], strings(octopus_release_args(&spec)));
        assert!(calls[0].contains(&"S".to_owned()));
        let args = std::fs::read_to_string(scratch.path().join("args.txt")).expect("args");
        let env = std::fs::read_to_string(scratch.path().join("env.txt")).expect("env");
        assert!(!args.contains("sekret") && !args.contains("--apiKey"));
        assert!(env.contains("OCTOPUS_CLI_API_KEY=sekret"));
        assert!(!env.contains("OCTOPUS_API_KEY") && !env.contains("DX_LEAK_PROBE"));
        std::fs::remove_file(scratch.path().join("args.txt")).expect("rm");
        assert_eq!(
            octopus_live_publish(&octo, package, &octo_spec(&envs, ""), "sekret"),
            0
        );
        let args = std::fs::read_to_string(scratch.path().join("args.txt")).expect("args");
        assert!(!args.contains("--space"));
    }

    #[cfg(unix)]
    #[test]
    fn octopus_live_publish_stops_after_failed_push() {
        let scratch = scratch_dir();
        let octo = fake_octo(scratch.path(), "push");
        let code = octopus_live_publish(
            &octo,
            Path::new("p.tar.gz"),
            &octo_spec(&["Production".to_owned()], ""),
            "sekret",
        );
        assert_eq!(code, 1);
        assert_eq!(octo_calls(scratch.path()).len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn octopus_live_publish_surfaces_release_failure() {
        let scratch = scratch_dir();
        let octo = fake_octo(scratch.path(), "create-release");
        let code = octopus_live_publish(
            &octo,
            Path::new("p.tar.gz"),
            &octo_spec(&["Production".to_owned()], ""),
            "sekret",
        );
        assert_eq!(code, 1);
        assert_eq!(octo_calls(scratch.path()).len(), 2);
    }

    #[cfg(unix)]
    fn fake_npm(dir: &Path, exit: i32) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join("npm");
        let d = dir.display();
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{d}/args.txt'\nenv > '{d}/env.txt'\nexit {exit}\n"
        );
        std::fs::write(&path, script).expect("write npm");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    }

    #[cfg(unix)]
    #[test]
    fn npm_run_publish_passes_policy_and_keeps_the_token_out_of_args() {
        let scratch = scratch_dir();
        let npm = fake_npm(scratch.path(), 0);
        let args = npm_publish_args(
            Path::new("x.tgz"),
            "latest",
            "public",
            "https://registry.npmjs.org",
            "required",
        );
        let code = with_env(
            &[
                ("DX_LEAK_PROBE", Some("1")),
                ("NPM_TOKEN", Some("sekret")),
                ("GITHUB_TOKEN", Some("gh-sekret")),
                ("CI_JOB_TOKEN", Some("job-sekret")),
                ("CI_PROJECT_URL", Some("https://gitlab.example/p")),
                (
                    "ACTIONS_ID_TOKEN_REQUEST_URL",
                    Some("https://ci.example/token"),
                ),
            ],
            || npm_run_publish(&npm, "registry.npmjs.org", "sekret", &args),
        );
        assert_eq!(code, 0);
        let seen = std::fs::read_to_string(scratch.path().join("args.txt")).expect("args");
        assert!(seen.contains("--provenance\n"));
        assert!(seen.contains("--userconfig"));
        assert!(!seen.contains("sekret"));
        let env = std::fs::read_to_string(scratch.path().join("env.txt")).expect("env");
        assert!(!env.contains("DX_LEAK_PROBE"), "{env}");
        assert!(!env.contains("sekret"), "{env}");
        assert!(env
            .lines()
            .any(|line| line == "ACTIONS_ID_TOKEN_REQUEST_URL=https://ci.example/token"));
        assert!(env
            .lines()
            .any(|line| line == "CI_PROJECT_URL=https://gitlab.example/p"));
    }

    #[cfg(unix)]
    #[test]
    fn npm_run_publish_surfaces_tool_failure_and_refusal() {
        let scratch = scratch_dir();
        let args = npm_publish_args(
            Path::new("x.tgz"),
            "latest",
            "public",
            "https://r",
            "required",
        );
        let refused = fake_npm(scratch.path(), 1);
        assert_eq!(npm_run_publish(&refused, "r", "tok", &args), 1);
        let failed = fake_npm(scratch.path(), 7);
        assert_eq!(npm_run_publish(&failed, "r", "tok", &args), 7);
        assert_eq!(
            npm_run_publish(&scratch.path().join("absent"), "r", "tok", &args),
            1
        );
    }

    #[cfg(unix)]
    #[test]
    fn npm_deploy_main_requires_owner_approval_for_live_publish() {
        if env_test_in_child("tests::npm_deploy_main_requires_owner_approval_for_live_publish") {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = stage_npm_demo(scratch.path(), "none");
        let bin = scratch.path().join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        fake_npm(&bin, 0);
        let args = bin.join("args.txt");
        let path = format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        for (approved, want) in [(None, 1), (Some("0"), 1), (Some("true"), 1), (Some("1"), 0)] {
            let outdir = tempfile_stage_dir("dx-deploy-npm-").expect("outdir");
            let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
            with_env(
                &[
                    ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                    ("RUNFILES_MANIFEST_FILE", None),
                    ("BUILD_WORKSPACE_DIRECTORY", None),
                    ("PATH", Some(path.as_str())),
                    ("NPM_PUBLISH_LIVE", Some("1")),
                    ("NPM_PUBLISH_DRY_RUN", None),
                    ("NPM_TOKEN", Some("sekret")),
                    ("NPM_PUBLISH_APPROVED", approved),
                ],
                || {
                    assert_eq!(
                        npm_deploy_main(&argv, "npm_demo.feed.json", "npm_demo.tgz"),
                        want,
                        "approval {approved:?}"
                    );
                },
            );
            std::fs::remove_dir_all(&outdir).expect("cleanup");
            if want == 1 {
                assert!(!args.exists(), "npm ran with approval {approved:?}");
            }
        }
        let seen = std::fs::read_to_string(&args).expect("npm ran once approved");
        assert!(seen.contains("publish"));
        assert!(!seen.contains("sekret"));
    }

    #[test]
    fn npm_deploy_main_refuses_required_provenance_without_identity() {
        if env_test_in_child("tests::npm_deploy_main_refuses_required_provenance_without_identity")
        {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = stage_npm_demo(scratch.path(), "required");
        let outdir = tempfile_stage_dir("dx-deploy-npm-").expect("outdir");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                ("BUILD_WORKSPACE_DIRECTORY", None),
                ("NPM_PUBLISH_LIVE", Some("1")),
                ("NPM_TOKEN", Some("tok")),
                ("NPM_PUBLISH_APPROVED", Some("1")),
                ("ACTIONS_ID_TOKEN_REQUEST_URL", None),
                ("SIGSTORE_ID_TOKEN", None),
            ],
            || {
                assert_eq!(
                    npm_deploy_main(&argv, "npm_demo.feed.json", "npm_demo.tgz"),
                    1
                );
            },
        );
        std::fs::remove_dir_all(&outdir).expect("cleanup");
    }

    #[test]
    fn member_name_error_accepts_layout_names() {
        for name in [
            "consumer_app",
            "NOTICE",
            "IDENTITY.txt",
            "resources/banner.txt",
        ] {
            assert!(member_name_error(name).is_none(), "{name} must be accepted");
        }
    }

    #[test]
    fn member_name_error_rejects_escapes() {
        for name in [
            "",
            "/absolute",
            "../escape",
            "a/../escape",
            "a//empty",
            "a/./dot",
            "back\\slash",
            "a=b",
            "nul\0byte",
        ] {
            assert!(
                member_name_error(name).is_some(),
                "{name:?} must be rejected"
            );
        }
        assert!(member_name_error(&"n".repeat(101)).is_some());
    }

    #[test]
    fn ustar_boundaries() {
        let tail = "n".repeat(100);
        let prefix = "p".repeat(155);
        assert!(fits_ustar(&"n".repeat(100)));
        assert!(!fits_ustar(&"n".repeat(101)));
        assert!(fits_ustar(&format!("{prefix}/{tail}")));
        assert!(!fits_ustar(&format!("{prefix}p/{tail}")));
        assert!(!fits_ustar(&format!("{prefix}/{tail}n")));
        assert!(!fits_ustar(&"n".repeat(300)));
        let wide = "\u{e9}".repeat(60);
        assert!(!fits_ustar(&wide));
        assert!(fits_ustar(&format!("{wide}/x")));
        assert!(member_name_error(&format!("{prefix}/{tail}")).is_none());
        assert!(member_name_error(&format!("{prefix}/{tail}n")).is_some());
    }

    #[test]
    fn long_nested_resource_path_round_trips() {
        let name = "resources/textures/textures/textures/textures/textures/textures/textures/textures/textures/textures/icon.png";
        assert_eq!(name.len(), 108);
        let scratch = scratch_dir();
        let src = scratch.path().join("icon.png");
        std::fs::write(&src, b"\x89PNG\x00bytes").expect("write");
        let out = scratch.path().join("out.tar.gz");
        let again = scratch.path().join("again.tar.gz");
        archive_members(
            &[("a", src.as_path(), false), (name, src.as_path(), false)],
            &out,
        )
        .expect("archive");
        archive_members(
            &[(name, src.as_path(), false), ("a", src.as_path(), false)],
            &again,
        )
        .expect("again");
        assert_eq!(
            std::fs::read(&out).expect("out"),
            std::fs::read(&again).expect("again")
        );
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(
            std::fs::File::open(&out).expect("open"),
        ));
        let mut found = false;
        for entry in archive.entries().expect("entries") {
            let mut entry = entry.expect("entry");
            if entry.path().expect("path").to_str() == Some(name) {
                let mut body = Vec::new();
                std::io::Read::read_to_end(&mut entry, &mut body).expect("read");
                assert_eq!(body, b"\x89PNG\x00bytes");
                found = true;
            }
        }
        assert!(found);
    }

    #[test]
    fn archive_members_orders_deterministically() {
        let scratch = scratch_dir();
        let first = scratch.path().join("first.txt");
        std::fs::write(&first, b"first\n").expect("write first");
        let second = scratch.path().join("second.txt");
        std::fs::write(&second, b"second\n").expect("write second");
        let forward = scratch.path().join("forward.tar.gz");
        archive_members(
            &[
                ("b/second.txt", second.as_path(), false),
                ("a/first.txt", first.as_path(), false),
            ],
            &forward,
        )
        .expect("archive forward");
        let reverse = scratch.path().join("reverse.tar.gz");
        archive_members(
            &[
                ("a/first.txt", first.as_path(), false),
                ("b/second.txt", second.as_path(), false),
            ],
            &reverse,
        )
        .expect("archive reverse");
        assert_eq!(
            std::fs::read(&forward).expect("read forward"),
            std::fs::read(&reverse).expect("read reverse")
        );
    }

    #[test]
    fn archive_members_round_trips_named_modes() {
        let scratch = scratch_dir();
        let executable = scratch.path().join("app.bin");
        std::fs::write(&executable, b"app\n").expect("write app");
        let plain = scratch.path().join("banner.txt");
        std::fs::write(&plain, b"banner\n").expect("write banner");
        let out = scratch.path().join("members.tar.gz");
        archive_members(
            &[
                ("consumer_app", executable.as_path(), true),
                ("resources/banner.txt", plain.as_path(), false),
            ],
            &out,
        )
        .expect("archive members");
        let gz = std::fs::read(&out).expect("read gz");
        let mut decoder = flate2::read::GzDecoder::new(&gz[..]);
        let mut archive = tar::Archive::new(&mut decoder);
        let mut entries = archive.entries().expect("entries");
        let first = entries.next().expect("first entry").expect("read first");
        assert_eq!(
            first.path().expect("first path").to_str(),
            Some("consumer_app")
        );
        assert_eq!(first.header().mode().expect("first mode"), 0o755);
        let second = entries.next().expect("second entry").expect("read second");
        assert_eq!(
            second.path().expect("second path").to_str(),
            Some("resources/banner.txt")
        );
        assert_eq!(second.header().mode().expect("second mode"), 0o644);
        assert!(entries.next().is_none(), "want exactly two members");
    }

    #[test]
    fn archive_members_rejects_bad_inputs() {
        let scratch = scratch_dir();
        let plain = scratch.path().join("plain.txt");
        std::fs::write(&plain, b"plain\n").expect("write plain");
        let out = scratch.path().join("out.tar.gz");
        assert!(archive_members(&[], &out).is_err());
        assert!(archive_members(
            &[
                ("plain.txt", plain.as_path(), false),
                ("plain.txt", plain.as_path(), false)
            ],
            &out
        )
        .is_err());
        assert!(archive_members(&[("/absolute", plain.as_path(), false)], &out).is_err());
        assert!(archive_members(&[("../escape", plain.as_path(), false)], &out).is_err());
        let missing = scratch.path().join("does-not-exist");
        assert!(archive_members(&[("missing.txt", missing.as_path(), false)], &out).is_err());
    }

    #[test]
    fn archive_tree_is_byte_identical_across_runs_and_orders() {
        let scratch = scratch_dir();
        let first = scratch.path().join("first.tar.gz");
        let second = scratch.path().join("second.tar.gz");
        archive_tree(
            &[
                ("p/b.sh", b"#!/bin/sh\n", 0o755),
                ("p/a.txt", b"a\n", 0o644),
            ],
            &first,
        )
        .expect("first");
        archive_tree(
            &[
                ("p/a.txt", b"a\n", 0o644),
                ("p/b.sh", b"#!/bin/sh\n", 0o755),
            ],
            &second,
        )
        .expect("second");
        assert_eq!(
            std::fs::read(&first).expect("first"),
            std::fs::read(&second).expect("second")
        );
    }

    #[test]
    fn archive_tree_matches_archive_members_bytes() {
        let scratch = scratch_dir();
        let src = scratch.path().join("plain.txt");
        std::fs::write(&src, b"plain\n").expect("write");
        let members = scratch.path().join("members.tar.gz");
        let tree = scratch.path().join("tree.tar.gz");
        archive_members(&[("x/plain.txt", src.as_path(), false)], &members).expect("members");
        archive_tree(&[("x/plain.txt", b"plain\n", 0o644)], &tree).expect("tree");
        assert_eq!(
            std::fs::read(&members).expect("members"),
            std::fs::read(&tree).expect("tree")
        );
    }

    #[test]
    fn member_args_marks_named_members_executable() {
        let args: Vec<String> = ["--executable", "dx", "NOTICE=/n", "dx=/b/dx"]
            .iter()
            .map(|arg| (*arg).to_owned())
            .collect();
        assert_eq!(
            member_args(&args).expect("args"),
            vec![
                ("NOTICE".to_owned(), std::path::PathBuf::from("/n"), false),
                ("dx".to_owned(), std::path::PathBuf::from("/b/dx"), true),
            ]
        );
        let plain = vec!["a=x=y".to_owned()];
        assert_eq!(
            member_args(&plain).expect("plain"),
            vec![("a".to_owned(), std::path::PathBuf::from("x=y"), false)]
        );
    }

    #[test]
    fn member_args_rejects_bad_flags_and_pairs() {
        let parse = |args: &[&str]| {
            member_args(&args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>())
                .expect_err("want an error")
        };
        assert_eq!(
            parse(&["--executable", "dx", "NOTICE=/n"]),
            "--executable \"dx\" names no member"
        );
        assert_eq!(parse(&["--executable"]), "--executable needs a member name");
        assert_eq!(
            parse(&["--executable", "dx"]),
            "need at least one <name=path> member"
        );
        assert_eq!(parse(&["NOTICE"]), "member \"NOTICE\" must be <name=path>");
    }

    #[test]
    fn archive_members_writes_a_deterministic_zip_with_unix_modes() {
        let scratch = scratch_dir();
        let exe = scratch.path().join("dx.exe");
        std::fs::write(&exe, b"MZ app\n").expect("write exe");
        let notice = scratch.path().join("NOTICE");
        std::fs::write(&notice, b"notice\n").expect("write notice");
        let forward = scratch.path().join("forward.zip");
        let reverse = scratch.path().join("reverse.zip");
        archive_members(
            &[
                ("dx.exe", exe.as_path(), true),
                ("NOTICE", notice.as_path(), false),
            ],
            &forward,
        )
        .expect("forward");
        archive_members(
            &[
                ("NOTICE", notice.as_path(), false),
                ("dx.exe", exe.as_path(), true),
            ],
            &reverse,
        )
        .expect("reverse");
        let bytes = std::fs::read(&forward).expect("read forward");
        assert_eq!(bytes, std::fs::read(&reverse).expect("read reverse"));
        assert_eq!(&bytes[..4], b"PK\x03\x04");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("open zip");
        assert_eq!(archive.len(), 2);
        let mut seen = Vec::new();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).expect("entry");
            assert_eq!(entry.compression(), zip::CompressionMethod::Deflated);
            assert_eq!(entry.system(), zip::System::Unix);
            let stamp = entry.last_modified().expect("time");
            assert_eq!((stamp.year(), stamp.month(), stamp.day()), (1980, 1, 1));
            let mut body = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut body).expect("read entry");
            seen.push((
                entry.name().expect("name").into_owned(),
                entry.unix_mode().expect("mode") & 0o7777,
                body,
            ));
        }
        assert_eq!(
            seen,
            vec![
                ("NOTICE".to_owned(), 0o644, b"notice\n".to_vec()),
                ("dx.exe".to_owned(), 0o755, b"MZ app\n".to_vec()),
            ]
        );
    }

    #[test]
    fn archive_tree_rejects_bad_zip_members() {
        let scratch = scratch_dir();
        let out = scratch.path().join("out.zip");
        assert!(archive_tree(&[], &out).is_err());
        assert!(archive_tree(&[("../x", b"1", 0o644)], &out).is_err());
        assert!(archive_tree(&[("p/x", b"1", 0o644), ("p/x", b"2", 0o644)], &out).is_err());
        assert!(!out.exists());
    }

    #[test]
    fn archive_tree_accepts_equals_and_rejects_unsafe_names() {
        let scratch = scratch_dir();
        let out = scratch.path().join("out.tar.gz");
        archive_tree(&[("p/real_delim,a;b=c.py", b"x", 0o644)], &out).expect("equals");
        assert!(member_name_error("p/b=c.txt").is_some());
        assert!(archive_tree(&[], &out).is_err());
        for bad in ["", "/abs", "p/../x", "p/./x", "p//x", "p\\x"] {
            let error = archive_tree(&[(bad, b"x", 0o644)], &out).expect_err(bad);
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{bad:?}");
        }
        let error = archive_tree(&[("p/x", b"1", 0o644), ("p/x", b"2", 0o644)], &out)
            .expect_err("duplicate");
        assert!(error.to_string().contains("duplicate name"), "{error}");
    }

    #[test]
    fn archive_listing_round_trips_names_modes_and_digests() {
        let scratch = scratch_dir();
        let out = scratch.path().join("out.tar.gz");
        archive_tree(
            &[("p/b=c.txt", b"bc\n", 0o644), ("p/run.sh", b"run\n", 0o755)],
            &out,
        )
        .expect("archive");
        assert_eq!(
            archive_listing(&out).expect("listing"),
            vec![
                (
                    "p/b=c.txt".to_owned(),
                    0o644,
                    dx_digest::sha256_hex(b"bc\n")
                ),
                (
                    "p/run.sh".to_owned(),
                    0o755,
                    dx_digest::sha256_hex(b"run\n")
                ),
            ]
        );
        assert_eq!(
            archive_read_member(&out, "p/run.sh").expect("member"),
            b"run\n"
        );
        assert!(archive_read_member(&out, "p/missing").is_err());
    }

    #[test]
    fn archive_listing_rejects_links_duplicates_and_cut_tails() {
        let scratch = scratch_dir();
        let mut builder = tar::Builder::new(Vec::new());
        let mut link = tar::Header::new_ustar();
        link.set_entry_type(tar::EntryType::Symlink);
        link.set_size(0);
        builder
            .append_link(&mut link, "p/link", "target")
            .expect("link");
        let linked = scratch.path().join("link.tar.gz");
        std::fs::write(
            &linked,
            gzip_compress(&builder.into_inner().expect("tar")).expect("gz"),
        )
        .expect("write");
        let error = archive_listing(&linked).expect_err("symlink");
        assert!(error.to_string().contains("not a regular file"), "{error}");
        let dup = tar_archive([("p/x", &b"1"[..], 0o644), ("p/x", &b"2"[..], 0o644)].into_iter())
            .expect("tar");
        let duplicated = scratch.path().join("dup.tar.gz");
        std::fs::write(&duplicated, gzip_compress(&dup).expect("gz")).expect("write");
        let error = archive_listing(&duplicated).expect_err("duplicate");
        assert!(error.to_string().contains("duplicate member"), "{error}");
        let whole = scratch.path().join("whole.tar.gz");
        archive_tree(&[("p/x", &[7u8; 4096][..], 0o644)], &whole).expect("archive");
        let bytes = std::fs::read(&whole).expect("read");
        let cut = scratch.path().join("cut.tar.gz");
        std::fs::write(&cut, &bytes[..bytes.len() - 4]).expect("write");
        assert!(archive_listing(&cut).is_err());
    }

    fn zip_image(entries: &[(&str, Option<u32>, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (name, mode, data) in entries {
            let mut options = zip::write::SimpleFileOptions::default();
            if let Some(mode) = mode {
                options = options.unix_permissions(*mode);
            }
            writer.start_file(*name, options).expect("start");
            writer.write_all(data).expect("data");
        }
        writer.finish().expect("finish").into_inner()
    }

    #[test]
    fn archive_extract_round_trips_tar_gz_and_zip_with_modes() {
        for suffix in ["tar.gz", "zip"] {
            let scratch = scratch_dir();
            let out = scratch.path().join(format!("cell.{suffix}"));
            archive_tree(
                &[
                    ("dx", b"exe\n", 0o755),
                    ("NOTICE", b"notice\n", 0o644),
                    ("lib/run=time.dll", b"dll\n", 0o644),
                ],
                &out,
            )
            .expect("archive");
            let listed = archive_listing(&out).expect("listing");
            let dir = scratch.path().join("dx install \u{e9} \u{fc}");
            let extracted = archive_extract(&out, &dir).expect("extract");
            assert_eq!(extracted, listed, "{suffix}");
            assert_eq!(
                std::fs::read(dir.join("lib/run=time.dll")).expect("read"),
                b"dll\n"
            );
            assert_eq!(
                archive_read_member(&out, "NOTICE").expect("member"),
                b"notice\n"
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                let mode = |name: &str| {
                    std::fs::metadata(dir.join(name))
                        .expect("meta")
                        .permissions()
                        .mode()
                        & 0o7777
                };
                assert_eq!(mode("dx"), 0o755, "{suffix}");
                assert_eq!(mode("NOTICE"), 0o644, "{suffix}");
            }
            let error = archive_extract(&out, &dir).expect_err("dir exists");
            assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        }
    }

    #[test]
    fn archive_extract_rejects_traversal_duplicates_links_and_modeless_zip() {
        let scratch = scratch_dir();
        let cases: Vec<(&str, Vec<u8>, &str)> = vec![
            (
                "up.zip",
                zip_image(&[("../escape", Some(0o644), b"x")]),
                "must not contain '.' or '..'",
            ),
            (
                "abs.zip",
                zip_image(&[("/abs", Some(0o644), b"x")]),
                "must be relative",
            ),
            (
                "drive.zip",
                zip_image(&[("C:evil", Some(0o644), b"x")]),
                "must not contain ':'",
            ),
            (
                "link.zip",
                {
                    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
                    writer
                        .add_symlink("link", "/etc", zip::write::SimpleFileOptions::default())
                        .expect("symlink");
                    writer.finish().expect("finish").into_inner()
                },
                "not a regular file",
            ),
            (
                "dir.zip",
                {
                    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
                    writer
                        .add_directory("d/", zip::write::SimpleFileOptions::default())
                        .expect("dir");
                    writer.finish().expect("finish").into_inner()
                },
                "not a regular file",
            ),
            (
                "modeless.zip",
                {
                    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
                    writer
                        .start_file(
                            "x",
                            zip::write::SimpleFileOptions::default().system(zip::System::Dos),
                        )
                        .expect("start");
                    writer.finish().expect("finish").into_inner()
                },
                "has no unix mode",
            ),
            ("garbage.zip", b"not a zip".to_vec(), "archive listing"),
        ];
        for (name, bytes, want) in cases {
            let path = scratch.path().join(name);
            std::fs::write(&path, bytes).expect("write");
            let dir = scratch.path().join(format!("{name}.out"));
            let error = archive_extract(&path, &dir).expect_err(name);
            assert!(error.to_string().contains(want), "{name}: {error}");
            assert!(!scratch.path().join("escape").exists());
            assert!(!Path::new("/abs").exists());
        }
        let mut builder = tar::Builder::new(Vec::new());
        let mut link = tar::Header::new_ustar();
        link.set_entry_type(tar::EntryType::Symlink);
        link.set_size(0);
        builder
            .append_link(&mut link, "link", "/etc")
            .expect("link");
        let linked = scratch.path().join("link.tar.gz");
        std::fs::write(
            &linked,
            gzip_compress(&builder.into_inner().expect("tar")).expect("gz"),
        )
        .expect("write");
        let error = archive_extract(&linked, &scratch.path().join("tar.out")).expect_err("link");
        assert!(error.to_string().contains("not a regular file"), "{error}");
        let dup = tar_archive([("x", &b"1"[..], 0o644), ("x", &b"2"[..], 0o644)].into_iter())
            .expect("tar");
        let duplicated = scratch.path().join("dup.tar.gz");
        std::fs::write(&duplicated, gzip_compress(&dup).expect("gz")).expect("write");
        let error = archive_extract(&duplicated, &scratch.path().join("dup.out")).expect_err("dup");
        assert!(error.to_string().contains("duplicate member"), "{error}");
        let mut raw = tar::Header::new_gnu();
        raw.set_entry_type(tar::EntryType::Regular);
        raw.set_size(1);
        raw.set_mode(0o644);
        raw.as_gnu_mut().expect("gnu").name[..9].copy_from_slice(b"../escape");
        raw.set_cksum();
        let mut builder = tar::Builder::new(Vec::new());
        builder.append(&raw, &b"x"[..]).expect("append");
        let escaping = scratch.path().join("escape.tar.gz");
        std::fs::write(
            &escaping,
            gzip_compress(&builder.into_inner().expect("tar")).expect("gz"),
        )
        .expect("write");
        let error =
            archive_extract(&escaping, &scratch.path().join("esc.out")).expect_err("escape");
        assert!(error.to_string().contains("'..'"), "{error}");
        assert!(!scratch.path().join("escape").exists());
    }

    #[test]
    fn archive_image_writes_a_deterministic_docker_save_archive() {
        let scratch = scratch_dir();
        let plain = scratch.path().join("plain.tar");
        let gz = scratch.path().join("gz.tar.gz");
        std::fs::write(&plain, oci_test_layer(false)).expect("plain");
        std::fs::write(&gz, oci_test_layer(true)).expect("gz");
        let first = scratch.path().join("first.tar");
        let second = scratch.path().join("second.tar");
        let layers = [plain.as_path(), gz.as_path()];
        let id = archive_image(&layers, "linux/arm64", &["/app/hello"], &first).expect("first");
        let again = archive_image(&layers, "linux/arm64", &["/app/hello"], &second).expect("again");
        assert_eq!(id, again);
        assert_eq!(
            std::fs::read(&first).expect("first"),
            std::fs::read(&second).expect("second")
        );
        let archive = oci_read_archive(&first, None).expect("read");
        assert_eq!(archive.config_digest, id);
        let config: serde_json::Value = serde_json::from_slice(&archive.config).expect("config");
        assert_eq!(config["architecture"], "arm64");
        assert_eq!(config["os"], "linux");
        assert_eq!(config["created"], "1970-01-01T00:00:00Z");
        assert_eq!(
            config["config"]["Entrypoint"],
            serde_json::json!(["/app/hello"])
        );
        let tar_digest = format!("sha256:{}", dx_digest::sha256_hex(&oci_test_layer(false)));
        assert_eq!(
            config["rootfs"]["diff_ids"],
            serde_json::json!([tar_digest, tar_digest])
        );
        assert_eq!(archive.layers.len(), 2);
        assert_eq!(archive.layers[0].digest, tar_digest);
        assert_ne!(archive.layers[1].digest, tar_digest);
        let out = scratch.path().join("bad.tar");
        for (layers, platform) in [
            (&[][..], "linux/amd64"),
            (&layers[..], "windows/amd64"),
            (&layers[..], ""),
        ] {
            let error = archive_image(layers, platform, &[], &out).expect_err(platform);
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{platform}");
        }
        assert!(!out.exists());
        let zst = scratch.path().join("zst.tar.zst");
        let mut zstd = ZSTD_MAGIC.to_vec();
        zstd.extend_from_slice(b"not a real zstd frame");
        std::fs::write(&zst, zstd).expect("zst");
        let error = archive_image(&[plain.as_path(), zst.as_path()], "linux/amd64", &[], &out)
            .expect_err("zstd");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(
            error
                .to_string()
                .contains("unsupported layer compression zstd"),
            "{error}"
        );
        assert!(!out.exists());
    }
}
