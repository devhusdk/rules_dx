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
    assert!(
        oci_declared_image_id(&archive, "linux/amd64")
            .expect_err("empty")
            .contains("exactly one")
    );
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
    assert!(
        oci_declared_image_id(&archive, "linux/amd64")
            .expect_err("many")
            .contains("2 images")
    );
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
    assert!(
        oci_declared_image_id(&archive, "linux/amd64")
            .expect_err("missing config")
            .contains("missing its config")
    );
    oci_tar(
        &archive,
        &[
            ("manifest.json", b"[{\"Config\":\"a.json\"}]"),
            ("a.json", &config),
        ],
        &[],
    );
    assert!(
        oci_declared_image_id(&archive, "linux/amd64")
            .expect_err("no layers")
            .contains("missing Layers")
    );
    oci_tar(&archive, &[("manifest.json", b"{not json")], &[]);
    assert!(
        oci_declared_image_id(&archive, "linux/amd64")
            .expect_err("malformed")
            .contains("malformed manifest.json")
    );
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
    assert!(
        oci_declared_image_id(&archive, "linux/amd64")
            .expect_err("rootfs layer")
            .contains("no manifest.json or index.json")
    );
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

fn oci_index_members(
    fixture: &OciIndexFixture,
    extra: &[(&str, &[u8])],
    links: &[(&str, &str)],
) -> Vec<(String, Vec<u8>)> {
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
    members
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
    let members = oci_index_members(&fixture, extra, links);
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

fn oci_write_dir(dir: &Path, members: &[(String, Vec<u8>)], links: &[(&str, &str)]) {
    if dir.exists() {
        std::fs::remove_dir_all(dir).expect("clear dir");
    }
    std::fs::create_dir_all(dir).expect("dir");
    for (name, body) in members {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("parents");
        std::fs::write(path, body).expect("write member");
    }
    for (name, target) in links {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("parents");
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, path).expect("symlink");
        #[cfg(not(unix))]
        std::fs::copy(
            dir.join(name).parent().expect("parent").join(target),
            dir.join(name),
        )
        .expect("copy link target");
    }
}

fn oci_index_dir(
    dir: &Path,
    layers: &[Vec<u8>],
    architecture: &str,
    nest: bool,
    extra: &[(&str, &[u8])],
    links: &[(&str, &str)],
) -> OciIndexFixture {
    let fixture = oci_index_fixture(layers, architecture, nest);
    oci_write_dir(dir, &oci_index_members(&fixture, extra, links), links);
    fixture
}

#[test]
fn oci_archive_image_id_reads_oci_layout_directories() {
    let scratch = scratch_dir();
    let layers = [oci_test_layer(false), oci_test_layer(true)];
    for (name, nest) in [("flat", false), ("nested", true)] {
        let dir = scratch.path().join(name);
        let fixture = oci_index_dir(&dir, &layers, "amd64", nest, &[], &[]);
        let tar = scratch.path().join(format!("{name}.tar"));
        oci_index_tar(&tar, &layers, "amd64", nest, &[], &[]);
        assert_eq!(
            oci_declared_image_id(&dir, "linux/amd64").expect(name),
            oci_digest(&fixture.config),
            "{name}"
        );
        assert_eq!(
            oci_read_archive(&dir, None, None).expect(name),
            oci_read_archive(&tar, None, None).expect(name),
            "{name}"
        );
    }
    let dir = scratch.path().join("linked");
    let fixture = oci_index_fixture(&layers, "amd64", true);
    let manifest_blob = oci_blob(&fixture.manifest);
    let config_blob = oci_blob(&fixture.config);
    let layer_blob = oci_blob(&layers[1]);
    oci_index_dir(
        &dir,
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
        oci_declared_image_id(&dir, "linux/amd64").expect("linked"),
        oci_digest(&fixture.config)
    );
    let docker_manifest = serde_json::to_vec(&serde_json::json!([{
        "Config": oci_blob(&fixture.config),
        "Layers": layers.iter().map(|layer| oci_blob(layer)).collect::<Vec<_>>(),
    }]))
    .expect("docker manifest");
    let dir = scratch.path().join("docker25");
    oci_index_dir(
        &dir,
        &layers,
        "amd64",
        true,
        &[
            ("manifest.json", &docker_manifest),
            ("index.json", b"{not json"),
        ],
        &[],
    );
    assert_eq!(
        oci_declared_image_id(&dir, "linux/amd64").expect("docker25"),
        oci_digest(&fixture.config)
    );
}

#[test]
fn oci_archive_image_id_picks_the_deploy_platform_from_an_index() {
    let scratch = scratch_dir();
    let amd = oci_index_fixture(
        &[oci_test_layer(false), oci_test_layer(true)],
        "amd64",
        false,
    );
    let arm = oci_index_fixture(&[oci_test_layer(false)], "arm64", false);
    let mut by_name = std::collections::BTreeMap::new();
    for (name, body) in amd.members.iter().chain(arm.members.iter()) {
        if name != "index.json" {
            by_name.insert(name.clone(), body.clone());
        }
    }
    let image = |manifest: &[u8], architecture: &str| {
        let mut descriptor = oci_descriptor("application/vnd.oci.image.manifest.v1+json", manifest);
        descriptor["platform"] = serde_json::json!({"architecture": architecture, "os": "linux"});
        descriptor
    };
    let index = oci_index_body(&[
        image(amd.manifest.as_slice(), "amd64"),
        image(arm.manifest.as_slice(), "arm64"),
    ]);
    by_name.insert("index.json".to_owned(), index);
    let members: Vec<(String, Vec<u8>)> = by_name.into_iter().collect();
    let dir = scratch.path().join("multi");
    oci_write_dir(&dir, &members, &[]);
    let tar = scratch.path().join("multi.tar");
    let files: Vec<(&str, &[u8])> = members
        .iter()
        .map(|(name, body)| (name.as_str(), body.as_slice()))
        .collect();
    oci_tar(&tar, &files, &[]);
    for path in [&dir, &tar] {
        assert_eq!(
            oci_declared_image_id(path, "linux/amd64").expect("amd64"),
            oci_digest(&amd.config),
            "{}",
            path.display()
        );
        assert_eq!(
            oci_declared_image_id(path, "linux/arm64").expect("arm64"),
            oci_digest(&arm.config),
            "{}",
            path.display()
        );
    }
    assert!(oci_read_archive(&dir, None, None).is_err());
    assert!(oci_read_archive(&tar, None, None).is_err());
}

#[test]
fn oci_archive_image_id_rejects_unsupported_oci_layout_directories() {
    let scratch = scratch_dir();
    let dir = scratch.path().join("layout");
    let layers = [oci_test_layer(false)];
    let fixture = oci_index_fixture(&layers, "amd64", false);
    let image = oci_descriptor(
        "application/vnd.oci.image.manifest.v1+json",
        &fixture.manifest,
    );
    let other = oci_descriptor("application/vnd.oci.image.manifest.v1+json", b"other");
    let two = oci_index_body(&[image.clone(), other]);
    let gone = oci_index_body(&[oci_descriptor(
        "application/vnd.oci.image.manifest.v1+json",
        b"gone",
    )]);
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
    let layer_blob = oci_blob(&layers[0]);
    let docker = |config: &str| {
        serde_json::to_vec(&serde_json::json!([{
            "Config": config,
            "Layers": [layer_blob.clone()],
        }]))
        .expect("docker manifest")
    };
    let (dotdot, drive, backslash) = (docker("../x"), docker("C:/x"), docker("a\\b"));
    let cases: Vec<(&str, Vec<(&str, &[u8])>, &str)> = vec![
        (
            "two images",
            vec![("index.json", two.as_slice())],
            "2 images",
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
        ("too deep", deep, "deeper than 8 levels"),
        (
            "dotdot",
            vec![("manifest.json", dotdot.as_slice())],
            "path '../x' escapes the archive",
        ),
        (
            "drive",
            vec![("manifest.json", drive.as_slice())],
            "missing its config blob 'C:/x'",
        ),
        (
            "backslash",
            vec![("manifest.json", backslash.as_slice())],
            "missing its config blob 'a\\b'",
        ),
    ];
    for (case, extra, want) in cases {
        oci_index_dir(&dir, &layers, "amd64", false, &extra, &[]);
        let error = oci_declared_image_id(&dir, "linux/amd64").expect_err(case);
        assert!(error.contains(want), "{case}: {error}");
    }
    std::fs::remove_dir_all(&dir).expect("clear");
    std::fs::create_dir_all(&dir).expect("empty dir");
    assert_eq!(
        oci_declared_image_id(&dir, "linux/amd64").expect_err("empty"),
        "image directory has no index.json or manifest.json; want an OCI image layout directory"
    );
    let bare = scratch.path().join("layout.bin");
    std::fs::write(&bare, b"image").expect("bare");
    assert_eq!(
        oci_declared_image_id(&bare, "linux/amd64").expect_err("suffix"),
        format!(
            "oci image: want a .tar or .tar.gz archive or an OCI layout directory, got '{}'",
            bare.display()
        )
    );
}

#[test]
fn oci_layout_reads_oci_layout_directories() {
    let scratch = scratch_dir();
    let layers = [oci_test_layer(false), oci_test_layer(true)];
    let tar = scratch.path().join("index.tar");
    oci_index_tar(&tar, &layers, "amd64", true, &[], &[]);
    let dir = scratch.path().join("index");
    oci_index_dir(&dir, &layers, "amd64", true, &[], &[]);
    let mut built = Vec::new();
    for (name, image) in [("tar", &tar), ("dir", &dir)] {
        let outdir = scratch.path().join(format!("out-{name}"));
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
    }
    assert_eq!(built[0], built[1]);
    let arm = scratch.path().join("arm");
    oci_index_dir(&arm, &layers, "arm64", false, &[], &[]);
    let outdir = scratch.path().join("out-arm");
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
    assert!(
        error.to_string().contains("image config is 'linux/arm64'"),
        "{error}"
    );
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
        descriptor["annotations"] = serde_json::json!({"org.opencontainers.image.ref.name": tag});
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
fn oci_paths_reject_invalid_zstd() {
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
        ("layer", layer, "is not valid zstd"),
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

fn zstd_frame(data: &[u8]) -> Vec<u8> {
    ruzstd::encoding::compress_to_vec(data, ruzstd::encoding::CompressionLevel::Fastest)
}

fn skippable_frame(length: u32, body: &[u8]) -> Vec<u8> {
    let mut frame = vec![0x50, 0x2A, 0x4D, 0x18];
    frame.extend_from_slice(&length.to_le_bytes());
    frame.extend_from_slice(body);
    frame
}

#[test]
fn oci_read_layer_hashes_every_zstd_frame() {
    let (a, b) = (b"first frame data".to_vec(), b"second frame data".to_vec());
    let joined = [a.clone(), b.clone()].concat();
    let want = format!("sha256:{}", dx_digest::sha256_hex(&joined));
    let plain = [zstd_frame(&a), zstd_frame(&b)].concat();
    let skipped = [zstd_frame(&a), skippable_frame(4, b"junk"), zstd_frame(&b)].concat();
    for (name, blob) in [("plain", plain), ("skippable", skipped)] {
        let layer = oci_read_layer(blob.as_slice(), name, None).expect(name);
        assert_eq!(layer.diff_id, want, "{name}");
    }
}

#[test]
fn oci_read_layer_rejects_bad_zstd_tails() {
    let good = zstd_frame(b"frame");
    let mut garbage = good.clone();
    garbage.extend_from_slice(b"trailing garbage");
    let mut short = good;
    short.extend_from_slice(&skippable_frame(8, b"abc"));
    for (name, blob) in [("garbage", garbage), ("short", short)] {
        let error = oci_read_layer(blob.as_slice(), name, None)
            .err()
            .expect(name);
        assert!(error.to_string().contains("is not valid zstd"), "{error}");
    }
}

#[test]
fn oci_paths_accept_multi_frame_zstd_layer() {
    let scratch = scratch_dir();
    let tar = oci_test_layer(false);
    let (head, tail) = tar.split_at(tar.len() / 2);
    let zstd = [zstd_frame(head), zstd_frame(tail)].concat();
    let config = oci_config("amd64", &[&tar]);
    let image = scratch.path().join("multi_frame.tar");
    oci_tar(
        &image,
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
    let outdir = scratch.path().join("out");
    std::fs::create_dir_all(&outdir).expect("outdir");
    oci_build_layout(
        &image,
        &outdir,
        "registry.invalid",
        "demo/app",
        "1.0.0",
        "linux/amd64",
    )
    .expect("multi-frame layer");
}

#[test]
fn archive_image_hashes_every_zstd_frame() {
    let scratch = scratch_dir();
    let tar = oci_test_layer(false);
    let (head, tail) = tar.split_at(tar.len() / 2);
    let layer = scratch.path().join("multi.tar.zst");
    std::fs::write(&layer, [zstd_frame(head), zstd_frame(tail)].concat()).expect("layer");
    let out = scratch.path().join("out.tar");
    archive_image(&[layer.as_path()], "linux/amd64", &[], &out).expect("archive");
    let archive = oci_read_archive(&out, None, None).expect("read");
    let config: serde_json::Value = serde_json::from_slice(&archive.config).expect("config");
    assert_eq!(
        config["rootfs"]["diff_ids"][0],
        format!("sha256:{}", dx_digest::sha256_hex(&tar)).as_str()
    );
}

#[test]
fn oci_paths_accept_zstd_layer() {
    let scratch = scratch_dir();
    let tar = oci_test_layer(false);
    let zstd = ruzstd::encoding::compress_to_vec(
        tar.as_slice(),
        ruzstd::encoding::CompressionLevel::Fastest,
    );
    let config = oci_config("amd64", &[&tar]);
    let image = scratch.path().join("zstd_layer.tar");
    oci_tar(
        &image,
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
    let outdir = scratch.path().join("out");
    std::fs::create_dir_all(&outdir).expect("outdir");
    oci_build_layout(
        &image,
        &outdir,
        "registry.invalid",
        "demo/app",
        "1.0.0",
        "linux/amd64",
    )
    .expect("zstd layer");
    let blob = outdir
        .join("blobs/sha256")
        .join(dx_digest::sha256_hex(&zstd));
    assert!(blob.exists() || std::fs::read_dir(&outdir).expect("outdir").next().is_some());
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
    let plain =
        tar_archive([("etc/demo.conf", b"demo=1\n".as_slice(), 0o644)].into_iter()).expect("tar");
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
    let other = tar_archive([("other", b"other\n".as_slice(), 0o644)].into_iter()).expect("tar");
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
    let manifest_two = b"[{\"Config\":\"c.json\",\"Layers\":[\"a/layer.tar\",\"b/layer.tar\"]}]";
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
    let archive = oci_read_archive(&first, None, None).expect("read");
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
    let good = scratch.path().join("good.tar.zst");
    let plain_bytes = std::fs::read(&plain).expect("plain");
    std::fs::write(
        &good,
        ruzstd::encoding::compress_to_vec(
            plain_bytes.as_slice(),
            ruzstd::encoding::CompressionLevel::Fastest,
        ),
    )
    .expect("good zst");
    archive_image(&[good.as_path()], "linux/amd64", &[], &out).expect("valid zstd layer");
    std::fs::remove_file(&out).expect("remove");
    let zst = scratch.path().join("zst.tar.zst");
    let mut zstd = ZSTD_MAGIC.to_vec();
    zstd.extend_from_slice(b"not a real zstd frame");
    std::fs::write(&zst, zstd).expect("zst");
    let error = archive_image(&[plain.as_path(), zst.as_path()], "linux/amd64", &[], &out)
        .expect_err("zstd");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("is not valid zstd"), "{error}");
    assert!(!out.exists());
}
