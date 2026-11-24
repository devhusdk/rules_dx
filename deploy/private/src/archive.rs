use crate::run::{DeployRun, deploy_build_mode};
use crate::stage::*;
use std::io::{self, BufRead, Read, Write};
use std::path::Path;

#[cfg(test)]
pub(crate) const RECORDSIZE: usize = 20 * 512;

const USTAR_LIMIT: &str = "must be at most 100 bytes, or split at a '/' into a prefix of at most 155 bytes and a name of at most 100 bytes, with no NUL";

pub(crate) fn tar_archive<'a>(
    members: impl Iterator<Item = (&'a str, &'a [u8], u32)>,
) -> io::Result<Vec<u8>> {
    let mut writer = dx_archive::TarWriter::new();
    for (name, data, mode) in members {
        writer.add_file(name, data, mode)?;
    }
    writer.finish_tar()
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

fn name_error(name: &str, allow_equals: bool) -> Option<String> {
    if name.is_empty() {
        return Some("archive member name must not be empty".to_owned());
    }
    if name.contains('\0') || !dx_archive::fits_ustar(name) {
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
        if let [first, second] = pair
            && first.0 == second.0
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("archive members: duplicate name {:?}", first.0),
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
        dx_archive::gzip(&tar_archive(ordered.into_iter())?)?
    };
    std::fs::write(dst, image)?;
    Ok(())
}

pub(crate) fn zip_archive(members: &[(&str, &[u8], u32)]) -> io::Result<Vec<u8>> {
    let mut writer = dx_archive::ZipWriter::new();
    for (name, data, mode) in members {
        writer.add_file(name, data, *mode)?;
    }
    writer.finish()
}

fn listing_error(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn is_zip(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "zip")
}

pub(crate) const LISTING_LIMITS: dx_archive::Limits = dx_archive::Limits {
    max_entries: usize::MAX,
    max_entry_bytes: u64::MAX,
    max_total_bytes: u64::MAX,
};

fn archive_entries<T>(
    path: &Path,
    limits: dx_archive::Limits,
    mut visit: impl FnMut(String, u32, &mut dyn Read) -> io::Result<Option<T>>,
) -> io::Result<Vec<T>> {
    let file = std::fs::File::open(path)?;
    let format = if is_zip(path) {
        dx_archive::Format::Zip
    } else {
        dx_archive::Format::TarGz
    };
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    dx_archive::visit_entries(file, format, limits, |entry, body| {
        let name = entry.name;
        if entry.kind != dx_archive::EntryKind::File {
            return Err(listing_error(format!(
                "member {name:?} is not a regular file"
            )));
        }
        let Some(mode) = entry.mode else {
            return Err(listing_error(format!("member {name:?} has no unix mode")));
        };
        if !seen.insert(name.to_owned()) {
            return Err(listing_error(format!("duplicate member {name:?}")));
        }
        if let Some(item) = visit(name.to_owned(), mode, body)? {
            out.push(item);
        }
        Ok(())
    })
    .map_err(|e| io::Error::new(e.kind(), format!("archive listing: {e}")))?;
    Ok(out)
}

pub fn archive_listing(path: &Path) -> io::Result<Vec<(String, u32, String)>> {
    archive_entries(path, LISTING_LIMITS, |name, mode, body| {
        Ok(Some((name, mode, dx_digest::sha256_reader_hex(body)?)))
    })
}

pub fn archive_read_member(path: &Path, member: &str) -> io::Result<Vec<u8>> {
    let mut found = archive_entries(path, LISTING_LIMITS, |name, _, body| {
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

pub fn archive_extract(path: &Path, dir: &Path) -> io::Result<Vec<(String, u32, String)>> {
    archive_extract_capped(path, dir, 512 << 20, 1 << 30)
}

struct HashingFile {
    file: std::fs::File,
    hash: dx_digest::Sha256Writer,
}

impl Write for HashingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file.write_all(buf)?;
        self.hash.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[expect(
    clippy::create_dir,
    reason = "extraction fails when the dir already exists"
)]
fn archive_extract_capped(
    path: &Path,
    dir: &Path,
    max_entry_bytes: u64,
    max_total_bytes: u64,
) -> io::Result<Vec<(String, u32, String)>> {
    std::fs::create_dir_all(dir.parent().unwrap_or(dir))?;
    std::fs::create_dir(dir)?;
    let limits = dx_archive::Limits {
        max_entry_bytes,
        max_total_bytes,
        ..LISTING_LIMITS
    };
    archive_entries(path, limits, |name, mode, body| {
        if let Some(reason) = name_error(&name, true) {
            return Err(listing_error(format!("archive extract: {reason}")));
        }
        let target = dir.join(&name);
        std::fs::create_dir_all(target.parent().unwrap_or(dir))?;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)?;
        let mut out = HashingFile {
            file,
            hash: dx_digest::Sha256Writer::new(),
        };
        io::copy(body, &mut out)?;
        out.flush()?;
        let HashingFile { file, hash } = out;
        drop(file);
        set_member_mode(&target, mode)?;
        Ok(Some((name, mode, hash.finish_hex())))
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

pub fn archive_file(src: &Path, dst: &Path) -> io::Result<()> {
    let name = basename(src)?;
    archive_members(&[(name.as_str(), src, true)], dst)
}

pub fn archive_bytes(data: &[u8], name: &str, executable: bool) -> io::Result<Vec<u8>> {
    let mode = if executable { 0o755 } else { 0o644 };
    let tar = tar_archive([(name, data, mode)].into_iter())?;
    dx_archive::gzip(&tar)
}

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
                dx_digest::sha256_reader_hex(zstd_multi_frame_reader(bytes.as_slice()))
                    .map_err(|error| invalid(&error))?
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
    if !dx_digest::verify(expected, &actual) {
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
            let build_mode = deploy_build_mode();
            println!("archive_deploy: released {app_name} (build_mode: {build_mode})");
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

#[derive(Clone, Copy)]
pub(crate) enum OciCompression {
    None,
    Gzip,
    Zstd,
}

pub(crate) fn oci_compression(head: &[u8]) -> OciCompression {
    if head.starts_with(&GZIP_MAGIC) {
        OciCompression::Gzip
    } else if head.starts_with(&ZSTD_MAGIC) {
        OciCompression::Zstd
    } else {
        OciCompression::None
    }
}

pub(crate) fn zstd_multi_frame_reader<R: BufRead>(source: R) -> impl Read {
    ZstdFrames {
        source,
        decoder: ruzstd::decoding::FrameDecoder::new(),
        active: false,
        frames: 0,
    }
}

struct ZstdFrames<R> {
    source: R,
    decoder: ruzstd::decoding::FrameDecoder,
    active: bool,
    frames: usize,
}

fn zstd_invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

impl<R: BufRead> Read for ZstdFrames<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        use ruzstd::decoding::BlockDecodingStrategy;
        use ruzstd::decoding::errors::{FrameDecoderError, ReadFrameHeaderError};
        loop {
            if self.active {
                if self.decoder.can_collect() > 0 {
                    return self.decoder.read(buf);
                }
                if self.decoder.is_finished() {
                    self.active = false;
                    continue;
                }
                self.decoder
                    .decode_blocks(&mut self.source, BlockDecodingStrategy::UptoBlocks(1))
                    .map_err(zstd_invalid)?;
                continue;
            }
            if self.source.fill_buf()?.is_empty() {
                if self.frames == 0 {
                    return Err(zstd_invalid("no zstd frames"));
                }
                return Ok(0);
            }
            match self.decoder.reset(&mut self.source) {
                Ok(()) => {
                    self.active = true;
                    self.frames += 1;
                }
                Err(FrameDecoderError::ReadFrameHeaderError(ReadFrameHeaderError::SkipFrame {
                    length,
                    ..
                })) => {
                    let skipped = io::copy(
                        &mut (&mut self.source).take(u64::from(length)),
                        &mut io::sink(),
                    )?;
                    if skipped < u64::from(length) {
                        return Err(zstd_invalid("truncated skippable frame"));
                    }
                }
                Err(error) => return Err(zstd_invalid(error)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    fn duplicate_tar(name: &str) -> Vec<u8> {
        let mut tar = dx_archive_testing::RawTar::new();
        for body in [&b"1"[..], &b"2"[..]] {
            tar.add_file(name, body).expect("append");
        }
        tar.finish().expect("tar")
    }

    #[test]
    fn archive_bytes_match_python_golden() {
        let gz = archive_bytes(b"hello world\n", "hello", true).expect("archive");
        assert_eq!(gz.len(), 110);
        assert_eq!(
            &gz[0..10],
            &[0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0xff]
        );
        let mut gunzip = flate2::read::GzDecoder::new(&gz[..]);
        let mut tar = Vec::new();
        gunzip.read_to_end(&mut tar).expect("decode");
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
        let upper_checksum = scratch.path().join("upper.sha256");
        std::fs::write(
            &upper_checksum,
            format!("{}  release_demo.tar.gz\n", digest.to_uppercase()).as_bytes(),
        )
        .expect("write upper");
        assert!(archive_stage_release(&app, &tarball, &upper_checksum, &outdir).is_ok());
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
        let mut found = false;
        dx_archive::visit(
            std::fs::File::open(&out).expect("open"),
            dx_archive::Format::TarGz,
            dx_archive::Limits::default(),
            |entry, _, body| {
                if entry == name {
                    let mut bytes = Vec::new();
                    body.read_to_end(&mut bytes)?;
                    assert_eq!(bytes, b"\x89PNG\x00bytes");
                    found = true;
                }
                Ok(())
            },
        )
        .expect("entries");
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
        let mut seen = Vec::new();
        dx_archive::visit_entries(
            std::fs::File::open(&out).expect("open"),
            dx_archive::Format::TarGz,
            dx_archive::Limits::default(),
            |entry, _| {
                seen.push((entry.name.to_owned(), entry.mode));
                Ok(())
            },
        )
        .expect("entries");
        assert_eq!(
            seen,
            vec![
                ("consumer_app".to_owned(), Some(0o755)),
                ("resources/banner.txt".to_owned(), Some(0o644)),
            ]
        );
    }

    #[test]
    fn archive_members_rejects_bad_inputs() {
        let scratch = scratch_dir();
        let plain = scratch.path().join("plain.txt");
        std::fs::write(&plain, b"plain\n").expect("write plain");
        let out = scratch.path().join("out.tar.gz");
        assert!(archive_members(&[], &out).is_err());
        assert!(
            archive_members(
                &[
                    ("plain.txt", plain.as_path(), false),
                    ("plain.txt", plain.as_path(), false)
                ],
                &out
            )
            .is_err()
        );
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
        let mut seen = Vec::new();
        dx_archive::visit_entries(
            std::io::Cursor::new(bytes),
            dx_archive::Format::Zip,
            dx_archive::Limits::default(),
            |entry, body| {
                let mut bytes = Vec::new();
                body.read_to_end(&mut bytes)?;
                seen.push((entry.name.to_owned(), entry.mode.expect("unix mode"), bytes));
                Ok(())
            },
        )
        .expect("entries");
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
        let mut tar = dx_archive_testing::RawTar::new();
        tar.add_symlink("p/link", "target").expect("link");
        let linked = scratch.path().join("link.tar.gz");
        std::fs::write(
            &linked,
            dx_archive::gzip(&tar.finish().expect("tar")).expect("gz"),
        )
        .expect("write");
        let error = archive_listing(&linked).expect_err("symlink");
        assert!(error.to_string().contains("not a regular file"), "{error}");
        let dup = duplicate_tar("p/x");
        let duplicated = scratch.path().join("dup.tar.gz");
        std::fs::write(&duplicated, dx_archive::gzip(&dup).expect("gz")).expect("write");
        let error = archive_listing(&duplicated).expect_err("duplicate");
        assert!(error.to_string().contains("duplicate member"), "{error}");
        let whole = scratch.path().join("whole.tar.gz");
        archive_tree(&[("p/x", &[7u8; 4096][..], 0o644)], &whole).expect("archive");
        let bytes = std::fs::read(&whole).expect("read");
        let cut = scratch.path().join("cut.tar.gz");
        std::fs::write(&cut, &bytes[..bytes.len() - 4]).expect("write");
        assert!(archive_listing(&cut).is_err());
    }

    fn zip_image(build: impl FnOnce(&mut dx_archive_testing::RawZip) -> io::Result<()>) -> Vec<u8> {
        let mut zip = dx_archive_testing::RawZip::new();
        build(&mut zip).expect("member");
        zip.finish().expect("finish")
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
    fn archive_extract_fails_over_member_and_total_caps() {
        for suffix in ["tar.gz", "zip"] {
            let scratch = scratch_dir();
            let out = scratch.path().join(format!("cell.{suffix}"));
            archive_tree(&[("a", b"12345", 0o644), ("b", b"12345", 0o644)], &out).expect("archive");
            let ok = archive_extract_capped(&out, &scratch.path().join("ok"), 5, 10);
            assert!(ok.is_ok(), "{suffix}");
            let member = archive_extract_capped(&out, &scratch.path().join("m"), 4, 100)
                .expect_err("member cap");
            assert!(member.to_string().contains("size cap"), "{suffix}");
            let total = archive_extract_capped(&out, &scratch.path().join("t"), 5, 9)
                .expect_err("total cap");
            assert!(total.to_string().contains("size cap"), "{suffix}");
        }
    }

    fn reject_zip_cases(scratch: &Path) {
        let cases: Vec<(&str, Vec<u8>, &str)> = vec![
            (
                "up.zip",
                zip_image(|zip| zip.add_file("../escape", b"x")),
                "must not contain '.' or '..'",
            ),
            (
                "abs.zip",
                zip_image(|zip| zip.add_file("/abs", b"x")),
                "must be relative",
            ),
            (
                "drive.zip",
                zip_image(|zip| zip.add_file("C:evil", b"x")),
                "must not contain ':'",
            ),
            (
                "link.zip",
                zip_image(|zip| zip.add_symlink("link", "/etc")),
                "not a regular file",
            ),
            (
                "dir.zip",
                zip_image(|zip| zip.add_dir("d")),
                "not a regular file",
            ),
            (
                "modeless.zip",
                zip_image(|zip| zip.add_dos_file("x", b"")),
                "has no unix mode",
            ),
            ("garbage.zip", b"not a zip".to_vec(), "archive listing"),
        ];
        for (name, bytes, want) in cases {
            let path = scratch.join(name);
            std::fs::write(&path, bytes).expect("write");
            let dir = scratch.join(format!("{name}.out"));
            let error = archive_extract(&path, &dir).expect_err(name);
            assert!(error.to_string().contains(want), "{name}: {error}");
            assert!(!scratch.join("escape").exists());
            assert!(!Path::new("/abs").exists());
        }
    }

    #[test]
    fn archive_extract_rejects_traversal_duplicates_links_and_modeless_zip() {
        let scratch = scratch_dir();
        reject_zip_cases(scratch.path());
        let mut tar = dx_archive_testing::RawTar::new();
        tar.add_symlink("link", "/etc").expect("link");
        let linked = scratch.path().join("link.tar.gz");
        std::fs::write(
            &linked,
            dx_archive::gzip(&tar.finish().expect("tar")).expect("gz"),
        )
        .expect("write");
        let error = archive_extract(&linked, &scratch.path().join("tar.out")).expect_err("link");
        assert!(error.to_string().contains("not a regular file"), "{error}");
        let dup = duplicate_tar("x");
        let duplicated = scratch.path().join("dup.tar.gz");
        std::fs::write(&duplicated, dx_archive::gzip(&dup).expect("gz")).expect("write");
        let error = archive_extract(&duplicated, &scratch.path().join("dup.out")).expect_err("dup");
        assert!(error.to_string().contains("duplicate member"), "{error}");
        let escaping = scratch.path().join("escape.tar.gz");
        std::fs::write(&escaping, raw_tar_gz("../escape", b"x")).expect("write");
        let error =
            archive_extract(&escaping, &scratch.path().join("esc.out")).expect_err("escape");
        assert!(error.to_string().contains("'..'"), "{error}");
        assert!(!scratch.path().join("escape").exists());
    }
}
