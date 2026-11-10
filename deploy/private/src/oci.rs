//! OCI image publisher: layout staging, docker-save reading and the registry push.

use crate::archive::*;
use crate::oci_scan::{OciChecks, ScanMode, run_trivy, scan_and_report};
use crate::registry;
use crate::run::{
    DeployRun, OwnerGate, deploy_build_mode, env_or_empty, placeholder_refusal,
    tree_bytes,
};
use std::io::{self, Read, Write};
use std::path::Path;

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
    if !(src_str.ends_with(".tar") || src_str.ends_with(".tar.gz") || image.is_dir()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "oci image: want a .tar or .tar.gz archive or an OCI layout directory, got '{src_str}'"
            ),
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

/// Writes the OCI layout for one image archive into `dst`, tagged with `reference`.
pub fn archive_oci_layout(
    image: &Path,
    platform: &str,
    reference: &str,
    dst: &Path,
) -> io::Result<()> {
    let (os, architecture) = oci_check_source(image, platform)?;
    oci_stage_layout(image, dst, reference, os, architecture)
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
    let archive =
        oci_read_archive(image, Some(&blobs), Some((os, architecture))).map_err(|error| {
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

/// One image input: a tar archive or an OCI layout directory.
#[derive(Clone, Copy)]
enum OciSource<'a> {
    Archive(&'a Path),
    Dir(&'a Path),
}

impl<'a> OciSource<'a> {
    fn new(image: &'a Path) -> Self {
        if image.is_dir() {
            OciSource::Dir(image)
        } else {
            OciSource::Archive(image)
        }
    }
}

/// Maps a member name to a path under a layout directory, or None if it is not a plain relative path.
fn oci_dir_member(root: &Path, name: &str) -> Option<std::path::PathBuf> {
    let mut path = root.to_path_buf();
    for segment in name.split('/') {
        if segment.contains('\\') || segment.contains(':') {
            return None;
        }
        let mut components = Path::new(segment).components();
        match (components.next(), components.next()) {
            (Some(std::path::Component::Normal(part)), None) => path.push(part),
            _ => return None,
        }
    }
    Some(path)
}

/// Opens one regular-file member of a layout directory, or None when it is absent.
fn oci_dir_open(root: &Path, name: &str) -> io::Result<Option<std::fs::File>> {
    let Some(path) = oci_dir_member(root, name) else {
        return Ok(None);
    };
    match std::fs::metadata(&path) {
        Ok(meta) if meta.is_file() => {}
        Ok(_) => return Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(io::Error::new(
                error.kind(),
                format!("{}: {error}", path.display()),
            ));
        }
    }
    std::fs::File::open(&path)
        .map(Some)
        .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", path.display())))
}

/// Visits the regular-file members named in `names`, streaming each one.
fn oci_each_member(
    source: OciSource<'_>,
    names: &[&str],
    mut visit: impl FnMut(&str, &mut dyn Read) -> io::Result<()>,
) -> io::Result<()> {
    match source {
        OciSource::Archive(image) => {
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
                if names.contains(&name.as_str()) {
                    visit(&name, &mut entry)?;
                }
            }
        }
        OciSource::Dir(root) => {
            let mut seen = std::collections::HashSet::new();
            for name in names {
                if !seen.insert(*name) {
                    continue;
                }
                if let Some(file) = oci_dir_open(root, name)? {
                    visit(name, &mut io::BufReader::new(file))?;
                }
            }
        }
    }
    Ok(())
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
        OciCompression::Zstd => dx_digest::sha256_reader_hex(zstd_multi_frame_reader(
            io::BufReader::new(head.chain(&mut tee)),
        ))
        .map_err(|error| {
            oci_invalid(format!(
                "image archive layer '{name}' is not valid zstd: {error}"
            ))
        })?,
        OciCompression::None => dx_digest::sha256_reader_hex(head.chain(&mut tee))?,
    };
    io::copy(&mut tee, &mut io::sink())?;
    Ok(OciLayer {
        digest: format!("sha256:{}", tee.hash.finish_hex()),
        size: tee.size,
        media_type: match compression {
            OciCompression::Gzip => "application/vnd.oci.image.layer.v1.tar+gzip",
            OciCompression::Zstd => "application/vnd.oci.image.layer.v1.tar+zstd",
            OciCompression::None => "application/vnd.oci.image.layer.v1.tar",
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

const OCI_LAYER_TYPES: [&str; 5] = [
    "application/vnd.oci.image.layer.v1.tar",
    "application/vnd.oci.image.layer.v1.tar+gzip",
    "application/vnd.oci.image.layer.v1.tar+zstd",
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
fn oci_scan(source: OciSource<'_>) -> io::Result<OciScan> {
    let mut scan = OciScan {
        links: OciLinks::new(),
        manifest_json: None,
        index_json: None,
    };
    let image = match source {
        OciSource::Archive(image) => image,
        OciSource::Dir(_) => {
            oci_each_member(source, &["manifest.json", "index.json"], |name, reader| {
                let mut body = Vec::new();
                reader.read_to_end(&mut body).map_err(oci_malformed)?;
                if name == "manifest.json" {
                    scan.manifest_json = Some(body);
                } else {
                    scan.index_json = Some(body);
                }
                Ok(())
            })?;
            return Ok(scan);
        }
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
    source: OciSource<'_>,
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
    let members: Vec<String> = wanted.keys().cloned().collect();
    let members: Vec<&str> = members.iter().map(String::as_str).collect();
    let mut found = std::collections::HashMap::new();
    oci_each_member(source, &members, |name, reader| {
        let Some(requested) = wanted.remove(name) else {
            return Ok(());
        };
        let mut body = Vec::new();
        reader.read_to_end(&mut body).map_err(oci_malformed)?;
        for name in requested {
            found.insert(name.to_owned(), body.clone());
        }
        Ok(())
    })?;
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
    source: OciSource<'_>,
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
    let mut bodies = oci_read_members(source, links, &names)?;
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
fn oci_index_plan(
    source: OciSource<'_>,
    index: &[u8],
    links: &OciLinks,
    target: Option<(&str, &str)>,
) -> io::Result<OciPlan> {
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
        for body in oci_read_descriptors(source, links, &nested)? {
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
    if let Some((os, architecture)) = target {
        let matching: Vec<serde_json::Value> = images
            .iter()
            .filter(|descriptor| {
                descriptor["platform"]["os"] == os
                    && descriptor["platform"]["architecture"] == architecture
            })
            .cloned()
            .collect();
        if !matching.is_empty() {
            images = matching;
        }
    }
    if images.len() != 1 {
        return Err(oci_invalid(format!(
            "image archive holds {} images; want exactly one",
            images.len()
        )));
    }
    let body = oci_read_descriptors(source, links, &images)?.remove(0);
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
pub fn oci_read_archive(
    image: &Path,
    blobs: Option<&Path>,
    target: Option<(&str, &str)>,
) -> io::Result<OciArchive> {
    let source = OciSource::new(image);
    let plan = oci_plan(source, target)?;
    oci_read_plan(source, &plan, blobs)
}

/// Finds the single image in an image archive or layout directory.
fn oci_plan(source: OciSource<'_>, target: Option<(&str, &str)>) -> io::Result<OciPlan> {
    let scan = oci_scan(source)?;
    if let Some(manifest) = &scan.manifest_json {
        oci_docker_plan(manifest, &scan.links)
    } else if let Some(index) = &scan.index_json {
        oci_index_plan(source, index, &scan.links, target)
    } else if let OciSource::Dir(_) = source {
        Err(oci_invalid(
            "image directory has no index.json or manifest.json; want an OCI image layout directory",
        ))
    } else {
        Err(oci_invalid(
            "image archive has no manifest.json or index.json; want a docker save or OCI layout archive, not a rootfs layer",
        ))
    }
}

/// Streams the config and layers named by a plan in one pass and checks their digests.
fn oci_read_plan(
    source: OciSource<'_>,
    plan: &OciPlan,
    blobs: Option<&Path>,
) -> io::Result<OciArchive> {
    let mut wanted: std::collections::HashMap<&str, Vec<usize>> = std::collections::HashMap::new();
    for (index, layer) in plan.layers.iter().enumerate() {
        wanted.entry(layer.as_str()).or_default().push(index);
    }
    let mut config: Option<Vec<u8>> = None;
    let mut layers: Vec<Option<OciLayer>> = vec![None; plan.layers.len()];
    let mut members: Vec<&str> = vec![plan.config.as_str()];
    members.extend(plan.layers.iter().map(String::as_str));
    oci_each_member(source, &members, |name, entry| {
        let is_config = name == plan.config && config.is_none();
        let pending: Vec<usize> = wanted
            .get(name)
            .map(|indexes| {
                indexes
                    .iter()
                    .copied()
                    .filter(|&index| layers[index].is_none())
                    .collect()
            })
            .unwrap_or_default();
        if !is_config && pending.is_empty() {
            return Ok(());
        }
        let read = if is_config {
            let mut body = Vec::new();
            entry.read_to_end(&mut body).map_err(oci_malformed)?;
            let read = if pending.is_empty() {
                None
            } else {
                Some(oci_copy_layer(io::Cursor::new(body.clone()), name, blobs)?)
            };
            config = Some(body);
            read
        } else {
            Some(oci_copy_layer(entry, name, blobs)?)
        };
        if let Some(layer) = read {
            for index in pending {
                layers[index] = Some(layer.clone());
            }
        }
        Ok(())
    })?;
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
    let archive = oci_read_archive(image, None, Some((os, architecture)))
        .map_err(|error| error.to_string())?;
    oci_check_config(&archive, os, architecture).map_err(|error| error.to_string())?;
    Ok(archive.config_digest)
}

/// Reads the config digest of the single manifest in a staged layout.
fn oci_layout_config_digest(layout: &Path) -> Result<String, String> {
    let read = |path: &Path| -> Result<serde_json::Value, String> {
        let bytes = std::fs::read(path).map_err(|error| format!("oci layout: {error}"))?;
        serde_json::from_slice(&bytes).map_err(|error| format!("oci layout: {error}"))
    };
    let index = read(&layout.join("index.json"))?;
    let digest = index["manifests"][0]["digest"].as_str().unwrap_or("");
    let manifest = read(
        &layout
            .join("blobs")
            .join("sha256")
            .join(digest.trim_start_matches("sha256:")),
    )?;
    manifest["config"]["digest"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "oci layout: manifest has no config digest".to_owned())
}

/// Target and credentials for one live push.
struct OciPush<'a> {
    registry: &'a str,
    repository: &'a str,
    tag: &'a str,
    platform: &'a str,
    creds: registry::Credentials<'a>,
}

fn oci_live_push(image: &Path, outdir: &Path, push: &OciPush<'_>) -> i32 {
    let target = match registry::RegistryRef::parse(push.registry, push.repository) {
        Ok(target) => target,
        Err(error) => {
            eprintln!("oci_deploy: {error}");
            return 1;
        }
    };
    oci_live_push_to(&crate::http::upload_agent(), &target, image, outdir, push)
}

/// Stages the layout and pushes exactly those bytes to the target.
fn oci_live_push_to(
    agent: &ureq::Agent,
    target: &registry::RegistryRef,
    image: &Path,
    outdir: &Path,
    push: &OciPush<'_>,
) -> i32 {
    let OciPush {
        registry,
        repository,
        tag,
        platform,
        creds,
    } = *push;
    let pushed = oci_build_layout(image, outdir, registry, repository, tag, platform)
        .map_err(|error| error.to_string())
        .and_then(|layout| {
            let config = oci_layout_config_digest(&layout)?;
            let digest = registry::push_layout(agent, target, tag, &layout, creds)?;
            Ok((digest, config))
        });
    match pushed {
        Ok((digest, config)) => {
            println!("oci_deploy: pushed {registry}/{repository}:{tag}@{digest} (config {config})");
            println!(
                "oci_deploy: signing-first: run cosign sign {registry}/{repository}@{digest} plus verify on the release trust root before updating scaffold refs"
            );
            0
        }
        Err(error) => {
            eprintln!("oci_deploy: {error}");
            1
        }
    }
}

/// Owner approval gate for oci_deploy.
pub(crate) const OCI_GATE: OwnerGate = OwnerGate {
    tool: "oci_deploy",
    action: "push",
    credentials: &["OCI_API_USER", "OCI_API_TOKEN"],
    approved: "OCI_PUBLISH_APPROVED",
};

/// Runs the OCI deploy tool.
pub fn oci_main(
    image_rloc: &str,
    registry: &str,
    repository: &str,
    tag: &str,
    platform: &str,
    checks: OciChecks,
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
    let Some(image) = run.resolve_anchored(image_rloc, "index.json") else {
        return 1;
    };
    let bytes = match tree_bytes(&image) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("oci_deploy: cannot measure {}: {error}", image.display());
            return 1;
        }
    };
    if OCI_GATE.requested(env_or_empty) {
        if let Some(error) = placeholder_refusal("oci_deploy", tag) {
            eprintln!("{error}");
            return 1;
        }
        let secrets = match OCI_GATE.authorize(env_or_empty) {
            Ok(secrets) => secrets,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        if checks.scan != ScanMode::Off {
            let staged = match oci_build_layout(&image, outdir, registry, repository, tag, platform)
            {
                Ok(layout) => layout,
                Err(error) => {
                    eprintln!("oci_deploy: {error}");
                    return 1;
                }
            };
            if scan_and_report(&staged, &checks, run_trivy) != 0 {
                eprintln!("oci_deploy: refusing to push an image with scan findings");
                return 1;
            }
        }
        return oci_live_push(
            &image,
            outdir,
            &OciPush {
                registry,
                repository,
                tag,
                platform,
                creds: registry::Credentials {
                    user: secrets.get("OCI_API_USER"),
                    secret: secrets.get("OCI_API_TOKEN"),
                },
            },
        );
    }
    match oci_build_layout(&image, outdir, registry, repository, tag, platform) {
        Ok(layout) => {
            let build_mode = deploy_build_mode();
            println!("oci_deploy: staged {registry}/{repository}:{tag} (build_mode: {build_mode})");
            println!("  layout: {}", layout.display());
            println!("  index: {}", layout.join("index.json").display());
            println!("  size: {bytes} bytes");
            scan_and_report(&layout, &checks, run_trivy)
        }
        Err(error) => {
            eprintln!("oci_deploy: {error}");
            1
        }
    }
}

#[cfg(test)]
#[path = "oci_tests.rs"]
mod tests;
