//! Deterministic tar, gzip and zip archives, plus the archive publisher.

use crate::run::{deploy_profile, DeployRun};
use crate::stage::*;
use std::io::{self, Read, Write};
use std::path::Path;

pub(crate) const RECORDSIZE: usize = 20 * 512;

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
pub(crate) fn ustar_header(name: &str, size: u64, mode: u32) -> io::Result<tar::Header> {
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
pub(crate) fn tar_archive<'a>(
    members: impl Iterator<Item = (&'a str, &'a [u8], u32)>,
) -> io::Result<Vec<u8>> {
    let mut builder = tar::Builder::new(Vec::new());
    for (name, data, mode) in members {
        let header = ustar_header(name, data.len() as u64, mode)?;
        builder.append(&header, data)?;
    }
    let mut image = builder.into_inner()?;
    image.resize(image.len().div_ceil(RECORDSIZE) * RECORDSIZE, 0);
    Ok(image)
}

pub(crate) fn gzip_compress(tar: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = flate2::GzBuilder::new()
        .mtime(0)
        .operating_system(255)
        .write(Vec::new(), flate2::Compression::best());
    encoder.write_all(tar)?;
    encoder.finish()
}

pub(crate) fn basename(path: &Path) -> io::Result<String> {
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
pub(crate) fn zip_archive(members: &[(&str, &[u8], u32)]) -> io::Result<Vec<u8>> {
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
                let invalid = |error: &dyn std::fmt::Display| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "archive image: layer {} is not valid zstd: {error}",
                            layer.display()
                        ),
                    )
                };
                let decoder = ruzstd::decoding::StreamingDecoder::new(bytes.as_slice())
                    .map_err(|error| invalid(&error))?;
                dx_digest::sha256_reader_hex(decoder).map_err(|error| invalid(&error))?
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

pub(crate) const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

pub(crate) const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];

/// Compression detected from the first bytes of an archive or layer.
#[derive(Clone, Copy)]
pub(crate) enum OciCompression {
    None,
    Gzip,
    Zstd,
}

/// Detects gzip or zstd from a stream prefix.
pub(crate) fn oci_compression(head: &[u8]) -> OciCompression {
    if head.starts_with(&GZIP_MAGIC) {
        OciCompression::Gzip
    } else if head.starts_with(&ZSTD_MAGIC) {
        OciCompression::Zstd
    } else {
        OciCompression::None
    }
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
}
