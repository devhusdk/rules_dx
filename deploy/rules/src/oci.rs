//! OCI image publisher: layout staging, docker-save reading and the registry push.

use crate::archive::*;
use crate::registry;
use crate::run::{deploy_profile, env_or_empty, DeployRun, OwnerGate};
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
            outdir,
            &OciPush {
                registry,
                repository,
                tag,
                platform,
                creds: registry::Credentials {
                    user: secrets.get("OCI_REGISTRY_USER"),
                    secret: secrets.get("OCI_REGISTRY_TOKEN"),
                },
            },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

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

    const FAKE_SECRET: &str = "s3cret-token-value";

    /// Runs one live push of `image` against a fake registry.
    fn fake_live_push(fake: &FakeRegistry, image: &Path, outdir: &Path, platform: &str) -> i32 {
        let target = registry::RegistryRef {
            endpoint: registry::Endpoint::for_test(&fake.origin),
            name: "demo/app".to_owned(),
        };
        oci_live_push_to(
            &crate::http::upload_agent_with_proxy(None),
            &target,
            image,
            outdir,
            &OciPush {
                registry: "registry.invalid",
                repository: "demo/app",
                tag: "1.2.3",
                platform,
                creds: registry::Credentials {
                    user: "me",
                    secret: FAKE_SECRET,
                },
            },
        )
    }

    #[test]
    fn oci_live_push_pushes_the_staged_layout_digest() {
        let scratch = scratch_dir();
        let (image, id) = oci_save(
            scratch.path(),
            "image.tar.gz",
            &[oci_test_layer(false), oci_test_layer(true)],
            "linux/amd64",
        );
        let dry = scratch.path().join("dry");
        std::fs::create_dir_all(&dry).expect("outdir");
        let layout = oci_build_layout(
            &image,
            &dry,
            "registry.invalid",
            "demo/app",
            "1.2.3",
            "linux/amd64",
        )
        .expect("layout");
        let (manifest, _) = oci_layout_json(&layout);
        assert_eq!(manifest["config"]["digest"].as_str(), Some(id.as_str()));
        let index: serde_json::Value =
            serde_json::from_slice(&std::fs::read(layout.join("index.json")).expect("index"))
                .expect("index json");
        let staged = index["manifests"][0]["digest"]
            .as_str()
            .expect("digest")
            .to_owned();
        let fake = FakeRegistry::start(FakeConfig {
            challenge: FakeChallenge::Bearer,
            ..FakeConfig::default()
        });
        let live = scratch.path().join("live");
        std::fs::create_dir_all(&live).expect("outdir");
        assert_eq!(fake_live_push(&fake, &image, &live, "linux/amd64"), 0);
        let requests = fake.requests();
        let manifest_put = requests.last().expect("manifest");
        assert_eq!(manifest_put.path, "/v2/demo/app/manifests/1.2.3");
        assert_eq!(manifest_put.body_digest, staged);
        let blobs: Vec<&str> = requests
            .iter()
            .filter(|r| r.method == "PUT" && r.path.contains("/blobs/uploads/"))
            .map(|r| r.body_digest.as_str())
            .collect();
        assert!(blobs.contains(&id.as_str()), "{blobs:?}");
    }

    #[test]
    fn oci_live_push_rejects_what_the_layout_rejects() {
        let scratch = scratch_dir();
        let layers = [oci_test_layer(false)];
        let (arm, _) = oci_save(scratch.path(), "arm.tar", &layers, "linux/arm64");
        let (tgz, _) = oci_save(scratch.path(), "image.tgz", &layers, "linux/amd64");
        let (amd, _) = oci_save(scratch.path(), "amd.tar", &layers, "linux/amd64");
        let cases = [
            ("arm64 image", &arm, "linux/amd64"),
            ("tgz suffix", &tgz, "linux/amd64"),
            ("s390x platform", &amd, "linux/s390x"),
        ];
        let fake = FakeRegistry::start(FakeConfig::default());
        for (name, image, platform) in cases {
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
            assert_eq!(fake_live_push(&fake, image, &outdir, platform), 1, "{name}");
            assert!(fake.requests().is_empty(), "{name}");
        }
        let outdir = scratch.path().join("arm_ok");
        std::fs::create_dir_all(&outdir).expect("outdir");
        assert_eq!(fake_live_push(&fake, &arm, &outdir, "linux/arm64"), 0);
        assert!(!fake.requests().is_empty());
    }

    #[test]
    fn oci_live_push_stops_before_manifest_on_blob_failure() {
        let scratch = scratch_dir();
        let (image, _) = oci_save(
            scratch.path(),
            "image.tar",
            &[oci_test_layer(false)],
            "linux/amd64",
        );
        let fake = FakeRegistry::start(FakeConfig {
            fail: vec![("PUT".to_owned(), "/blobs/uploads/".to_owned(), 500)],
            ..FakeConfig::default()
        });
        assert_eq!(
            fake_live_push(&fake, &image, scratch.path(), "linux/amd64"),
            1
        );
        let requests = fake.requests();
        assert!(requests.iter().any(|r| r.path.contains("/blobs/uploads/")));
        assert!(requests.iter().all(|r| !r.path.contains("/manifests/")));
    }

    #[test]
    fn oci_live_push_output_has_no_credentials() {
        const CHILD: &str = "DX_OCI_LIVE_PUSH_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let scratch = scratch_dir();
            let (image, _) = oci_save(
                scratch.path(),
                "image.tar",
                &[oci_test_layer(false)],
                "linux/amd64",
            );
            for (challenge, secret) in [
                (FakeChallenge::Bearer, FAKE_SECRET),
                (FakeChallenge::Bearer, "wrong"),
                (FakeChallenge::Basic, "wrong"),
            ] {
                let fake = FakeRegistry::start(FakeConfig {
                    challenge,
                    secret: secret.to_owned(),
                    ..FakeConfig::default()
                });
                let code = fake_live_push(&fake, &image, scratch.path(), "linux/amd64");
                assert_eq!(code, i32::from(secret != FAKE_SECRET));
            }
            return;
        }
        let run = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args([
                "oci::tests::oci_live_push_output_has_no_credentials",
                "--exact",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .expect("spawn child");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&run.stdout),
            String::from_utf8_lossy(&run.stderr)
        );
        assert!(run.status.success(), "{text}");
        assert!(text.contains("oci_deploy: pushed registry.invalid/demo/app:1.2.3@sha256:"));
        assert!(text.contains(registry::OCI_BAD_CREDENTIALS), "{text}");
        let basic = crate::http::basic_auth("me", FAKE_SECRET);
        for secret in [FAKE_SECRET, FAKE_TOKEN, basic.as_str(), &basic[6..]] {
            assert!(!text.contains(secret), "{text}");
        }
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
