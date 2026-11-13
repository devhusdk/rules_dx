use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn elf_needed(path: &Path) -> BTreeSet<String> {
    let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
    assert_eq!(&bytes[..4], b"\x7fELF", "{path:?}");
    let u16_at =
        |at: usize| u16::from_le_bytes(bytes[at..at + 2].try_into().expect("u16")) as usize;
    let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("u32"));
    let u64_at =
        |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().expect("u64")) as usize;
    let (table, size, count) = (u64_at(0x20), u16_at(0x36), u16_at(0x38));
    let headers: Vec<usize> = (0..count).map(|index| table + index * size).collect();
    let loads: Vec<(usize, usize, usize)> = headers
        .iter()
        .filter(|at| u32_at(**at) == 1)
        .map(|at| (u64_at(at + 16), u64_at(at + 8), u64_at(at + 32)))
        .collect();
    let to_offset = |address: usize| {
        loads
            .iter()
            .find(|(base, _, length)| address >= *base && address < base + length)
            .map(|(base, offset, _)| address - base + offset)
            .expect("mapped address")
    };
    let dynamic = headers
        .iter()
        .find(|at| u32_at(**at) == 2)
        .expect("dynamic segment");
    let (start, length) = (u64_at(dynamic + 8), u64_at(dynamic + 32));
    let mut strings = 0;
    let mut names = Vec::new();
    for entry in (start..start + length).step_by(16) {
        match u64_at(entry) {
            0 => break,
            1 => names.push(u64_at(entry + 8)),
            5 => strings = to_offset(u64_at(entry + 8)),
            _ => {}
        }
    }
    names
        .into_iter()
        .map(|name| {
            let from = strings + name;
            let end = bytes[from..].iter().position(|b| *b == 0).expect("NUL");
            String::from_utf8_lossy(&bytes[from..from + end]).into_owned()
        })
        .collect()
}

fn pinned_needs(table: &str) -> std::collections::BTreeMap<String, BTreeSet<String>> {
    let text = std::fs::read_to_string(dx_testing::env_file("DX_QT_NEEDS")).expect("needs tables");
    let tables: std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, BTreeSet<String>>,
    > = dx_testing::serde_json::from_str(&text).expect("needs JSON");
    tables.get(table).cloned().expect("table")
}

fn shared_objects(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("directory") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            shared_objects(&path, found);
        } else {
            found.push(path);
        }
    }
}

#[test]
fn the_pinned_dependency_tables_match_the_shipped_libraries() {
    let qmldir = std::fs::canonicalize(dx_testing::env_file("DX_QT_QMLDIR_PATH")).expect("qmldir");
    let cell = qmldir.ancestors().nth(3).expect("cell root").to_path_buf();
    let mut libraries = std::collections::BTreeMap::new();
    for entry in std::fs::read_dir(cell.join("lib")).expect("lib") {
        let path = entry.expect("entry").path();
        let name = file_name(&path.to_string_lossy()).to_owned();
        let version = name.rsplit_once(".so.").map(|(_, version)| version);
        if version.is_some_and(|version| version.chars().all(|c| c.is_ascii_digit())) {
            libraries.insert(name, path);
        }
    }
    let known = |path: &Path| {
        elf_needed(path)
            .into_iter()
            .filter(|dep| libraries.contains_key(dep))
            .collect::<BTreeSet<_>>()
    };
    let want: std::collections::BTreeMap<String, BTreeSet<String>> = libraries
        .iter()
        .map(|(name, path)| (name.clone(), known(path)))
        .collect();
    assert_eq!(pinned_needs("library"), want);

    let mut files = Vec::new();
    shared_objects(&cell.join("qml"), &mut files);
    let plugin_want: std::collections::BTreeMap<String, BTreeSet<String>> = files
        .iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "so"))
        .map(|path| {
            let relative = path.strip_prefix(cell.join("qml")).expect("below qml");
            (relative.to_string_lossy().into_owned(), known(path))
        })
        .collect();
    assert_eq!(pinned_needs("plugin"), plugin_want);

    let runtime_plugin_want: std::collections::BTreeMap<String, BTreeSet<String>> = [
        "imageformats/libqgif.so",
        "imageformats/libqico.so",
        "imageformats/libqjpeg.so",
        "platforms/libqminimal.so",
        "platforms/libqoffscreen.so",
        "platforms/libqwayland.so",
        "platforms/libqxcb.so",
        "platformthemes/libqxdgdesktopportal.so",
        "tls/libqcertonlybackend.so",
        "tls/libqopensslbackend.so",
        "wayland-decoration-client/libbradient.so",
        "wayland-graphics-integration-client/libqt-plugin-wayland-egl.so",
        "wayland-shell-integration/libxdg-shell.so",
        "xcbglintegrations/libqxcb-egl-integration.so",
        "xcbglintegrations/libqxcb-glx-integration.so",
    ]
    .into_iter()
    .map(|name| (name.to_owned(), known(&cell.join("plugins").join(name))))
    .collect();
    assert_eq!(pinned_needs("runtime_plugin"), runtime_plugin_want);
}
