mod runfiles_manifest;

#[test]
fn subject_matches_platform() {
    let key = std::env::var("DX_CONFORMANCE_SUBJECT").expect("DX_CONFORMANCE_SUBJECT must be set");
    let platform = std::env::var("DX_CONFORMANCE_TEXT").unwrap_or_default();
    match std::env::var("DX_CONFORMANCE_CROSS_RUNFILE") {
        Ok(name) => assert_runfile_image(&name, &platform),
        Err(_) => assert_image(&key, &platform),
    }
}

fn expected_image(platform: &str) -> (&'static str, &'static str) {
    let (os, cpu) = platform
        .split_once('_')
        .unwrap_or_else(|| panic!("unknown platform {platform}"));
    let format = match os {
        "linux" => "elf",
        "macos" => "macho",
        "windows" => "pe",
        "wasi" | "none" => "wasm",
        _ => panic!("unknown platform {platform}"),
    };
    let machine = match cpu {
        "arm64" => "aarch64",
        "x86_64" => "x86_64",
        "wasm32" => "wasm32",
        _ => panic!("unknown platform {platform}"),
    };
    (format, machine)
}

fn assert_image(key: &str, platform: &str) {
    let (format, machine) = expected_image(platform);
    let mut seen = Vec::new();
    for entry in key.split(' ').filter(|entry| !entry.is_empty()) {
        let path = dx_testing::rlocation(entry).expect("subject must resolve from runfiles");
        if entry.to_ascii_lowercase().ends_with(".msix") {
            assert_msix_image(entry, &path, platform);
            return;
        }
        let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("read {path:?}: {error}"));
        match dx_testing_image::inspect(&bytes) {
            Ok(images) => {
                assert!(!images.is_empty(), "{entry}: no image");
                for image in images {
                    assert_eq!(
                        (image.format.name(), image.machine.as_str()),
                        (format, machine),
                        "{entry}: format and machine for {platform}"
                    );
                }
                return;
            }
            Err(error) => seen.push(format!("{entry}: {error}")),
        }
    }
    panic!("no binary image among the subject files: {seen:?}");
}

fn msix_arch(platform: &str) -> &'static str {
    match expected_image(platform) {
        ("pe", "aarch64") => "arm64",
        ("pe", "x86_64") => "x64",
        _ => panic!("no msix ProcessorArchitecture for {platform}"),
    }
}

fn assert_msix_image(entry: &str, path: &std::path::Path, platform: &str) {
    let arch = msix_arch(platform);
    let (format, machine) = expected_image(platform);
    let package = dx_testing_msix::read(path).unwrap_or_else(|error| panic!("{entry}: {error}"));
    assert_eq!(
        package.arch, arch,
        "{entry}: manifest ProcessorArchitecture for {platform}"
    );
    let exe = &package.executable;
    let images = dx_testing_image::inspect(&package.exe)
        .unwrap_or_else(|error| panic!("{entry}: packed {exe}: {error}"));
    assert!(!images.is_empty(), "{entry}: packed {exe}: no image");
    for image in images {
        assert_eq!(
            (image.format.name(), image.machine.as_str()),
            (format, machine),
            "{entry}: packed {exe}: format and machine for {platform}"
        );
    }
}

fn assert_runfile_image(name: &str, platform: &str) {
    let tmp = std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR must be set");
    let srcdir = std::env::var("TEST_SRCDIR").unwrap_or_default();
    let runfiles = std::env::var("RUNFILES_DIR").unwrap_or(srcdir);
    let manifest = runfiles_manifest::manifest_file(&runfiles, std::path::Path::new(&tmp));
    let text = std::fs::read_to_string(&manifest).expect("runfiles manifest must be readable");
    let keys = runfile_keys(&text, name);
    assert!(!keys.is_empty(), "no runfile named {name} in {manifest:?}");
    let (format, machine) = expected_image(platform);
    let mut seen = Vec::new();
    let mut matched = 0;
    for key in keys {
        let path = dx_testing::rlocation(key).expect("runfile must resolve");
        let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("read {path:?}: {error}"));
        match dx_testing_image::inspect(&bytes) {
            Ok(images) => {
                assert!(!images.is_empty(), "{key}: no image");
                for image in images {
                    assert_eq!(
                        (image.format.name(), image.machine.as_str()),
                        (format, machine),
                        "{key}: format and machine for {platform}"
                    );
                }
                matched += 1;
            }
            Err(error) => seen.push(format!("{key}: {error}")),
        }
    }
    assert!(
        matched > 0,
        "no binary image among the {name} runfiles: {seen:?}"
    );
}

fn runfile_keys<'a>(manifest: &'a str, name: &str) -> Vec<&'a str> {
    manifest
        .lines()
        .map(|line| line.split_once(' ').map_or(line, |(key, _)| key))
        .filter(|key| {
            runfiles_manifest::launcher_stem(key.rsplit('/').next().unwrap_or(key)) == name
        })
        .collect()
}

#[test]
fn runfile_keys_match_file_stem() {
    let manifest = "_main/a/node_bin /x\nnodejs_linux_arm64/bin/nodejs/bin/node /y\nnodejs_windows_arm64/bin/nodejs/node.exe /z\n_main/node/greet.js /w\n";
    assert_eq!(
        runfile_keys(manifest, "node"),
        [
            "nodejs_linux_arm64/bin/nodejs/bin/node",
            "nodejs_windows_arm64/bin/nodejs/node.exe"
        ]
    );
}

#[test]
fn expected_image_names_format_and_machine() {
    assert_eq!(expected_image("linux_arm64"), ("elf", "aarch64"));
    assert_eq!(expected_image("macos_arm64"), ("macho", "aarch64"));
    assert_eq!(expected_image("windows_x86_64"), ("pe", "x86_64"));
    assert_eq!(expected_image("wasi_wasm32"), ("wasm", "wasm32"));
    assert_eq!(expected_image("none_wasm32"), ("wasm", "wasm32"));
}

#[test]
fn msix_arch_names_manifest_values() {
    assert_eq!(msix_arch("windows_arm64"), "arm64");
    assert_eq!(msix_arch("windows_x86_64"), "x64");
}

#[test]
#[should_panic(expected = "no msix ProcessorArchitecture for linux_arm64")]
fn msix_arch_rejects_non_windows() {
    msix_arch("linux_arm64");
}
