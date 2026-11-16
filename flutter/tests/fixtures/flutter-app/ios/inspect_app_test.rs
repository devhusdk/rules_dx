//! Checks the Flutter fixture iOS app bundle for the arm64 simulator.

use std::path::{Path, PathBuf};

const FAT_MAGIC: u32 = 0xcafe_babe;
const MH_MAGIC_64: u32 = 0xfeed_facf;
const CPU_TYPE_ARM64: u32 = 0x0100_000c;
const LC_BUILD_VERSION: u32 = 0x32;
const PLATFORM_IOSSIMULATOR: u32 = 7;

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"))
}

fn word(bytes: &[u8], at: usize) -> u32 {
    let slice: [u8; 4] = bytes[at..at + 4].try_into().expect("four bytes");
    u32::from_le_bytes(slice)
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn bundle(app: &str, scratch: &Path) -> PathBuf {
    if Path::new(app).extension().is_some_and(|end| end == "app") {
        return PathBuf::from(app);
    }
    let file = std::fs::File::open(app).unwrap_or_else(|error| panic!("open {app}: {error}"));
    zip::ZipArchive::new(file)
        .and_then(|mut archive| archive.extract(scratch))
        .unwrap_or_else(|error| panic!("unpack {app}: {error}"));
    std::fs::read_dir(scratch.join("Payload"))
        .unwrap_or_else(|error| panic!("{app} has no Payload: {error}"))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|end| end == "app"))
        .unwrap_or_else(|| panic!("{app} holds no Payload/*.app"))
}

fn big(bytes: &[u8], at: usize) -> u32 {
    let slice: [u8; 4] = bytes[at..at + 4].try_into().expect("four bytes");
    u32::from_be_bytes(slice)
}

/// The arm64 slice of a universal Mach-O, or `bytes` itself.
fn arm64_slice(bytes: &[u8]) -> Option<&[u8]> {
    if bytes.len() < 8 || big(bytes, 0) != FAT_MAGIC {
        return Some(bytes);
    }
    (0..big(bytes, 4) as usize).find_map(|index| {
        let at = 8 + index * 20;
        if at + 20 > bytes.len() || big(bytes, at) != CPU_TYPE_ARM64 {
            return None;
        }
        let (offset, size) = (big(bytes, at + 8) as usize, big(bytes, at + 12) as usize);
        bytes.get(offset..offset + size)
    })
}

/// The build platform of the arm64 Mach-O in `bytes`, or `None` when it holds none.
fn simulator_platform(bytes: &[u8]) -> Option<u32> {
    let bytes = arm64_slice(bytes)?;
    if bytes.len() < 32 || word(bytes, 0) != MH_MAGIC_64 || word(bytes, 4) != CPU_TYPE_ARM64 {
        return None;
    }
    let commands = word(bytes, 16);
    let mut at = 32;
    for _ in 0..commands {
        if at + 12 > bytes.len() {
            return None;
        }
        let (cmd, size) = (word(bytes, at), word(bytes, at + 4) as usize);
        if cmd == LC_BUILD_VERSION {
            return Some(word(bytes, at + 8));
        }
        if size == 0 {
            return None;
        }
        at += size;
    }
    None
}

#[test]
fn the_app_is_an_arm64_simulator_bundle() {
    let tmp = PathBuf::from(env("TEST_TMPDIR"));
    let app = bundle(&env("DX_IOS_APP"), &tmp);
    let bundle_id = env("DX_IOS_BUNDLE_ID");

    let info = read(&app.join("Info.plist"));
    assert!(
        info.windows(bundle_id.len()).any(|w| w == bundle_id.as_bytes()),
        "Info.plist does not name {bundle_id}"
    );

    for framework in ["Flutter", "App"] {
        let binary = app.join(format!("Frameworks/{framework}.framework/{framework}"));
        assert_eq!(
            simulator_platform(&read(&binary)),
            Some(PLATFORM_IOSSIMULATOR),
            "{} is not an arm64 iOS simulator Mach-O",
            binary.display()
        );
    }

    let executables: Vec<PathBuf> = std::fs::read_dir(&app)
        .expect("list the app bundle")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| simulator_platform(&read(path)).is_some())
        .collect();
    assert_eq!(executables.len(), 1, "main executables: {executables:?}");
    assert_eq!(
        simulator_platform(&read(&executables[0])),
        Some(PLATFORM_IOSSIMULATOR),
        "{} is not built for the iOS simulator",
        executables[0].display()
    );
}
