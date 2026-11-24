use std::path::Path;

const MANIFEST: &str = "AppxManifest.xml";
const MANIFEST_MAX_BYTES: u64 = 1 << 20;
const EXE_MAX_BYTES: u64 = 512 << 20;

#[derive(Debug)]
pub struct Package {
    pub arch: String,
    pub executable: String,
    pub exe: Vec<u8>,
}

pub fn read(path: &Path) -> Result<Package, String> {
    let manifest = member(path, MANIFEST, MANIFEST_MAX_BYTES)?;
    let manifest = String::from_utf8(manifest)
        .map_err(|error| format!("{}: {MANIFEST} is not UTF-8: {error}", path.display()))?;
    let arch = attribute(&manifest, "<Identity ", "ProcessorArchitecture").ok_or_else(|| {
        format!(
            "{}: {MANIFEST} has no Identity ProcessorArchitecture",
            path.display()
        )
    })?;
    let executable = attribute(&manifest, "<Application ", "Executable")
        .ok_or_else(|| {
            format!(
                "{}: {MANIFEST} has no Application Executable",
                path.display()
            )
        })?
        .replace('\\', "/");
    let exe = member(path, &executable, EXE_MAX_BYTES)?;
    Ok(Package {
        arch,
        executable,
        exe,
    })
}

fn member(path: &Path, name: &str, max_bytes: u64) -> Result<Vec<u8>, String> {
    dx_archive::read_zip_member(path, name, max_bytes)
        .map_err(|error| format!("{}: {error}", path.display()))?
        .ok_or_else(|| format!("{}: no member {name}", path.display()))
}

fn attribute(xml: &str, tag: &str, name: &str) -> Option<String> {
    let (_, rest) = xml.split_once(tag)?;
    let (element, _) = rest.split_once('>')?;
    let element = format!(" {element}");
    let (_, value) = element.split_once(&format!(" {name}=\""))?;
    let (value, _) = value.split_once('"')?;
    Some(unescape(value))
}

fn unescape(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_archive_testing::RawZip;

    fn manifest(arch: &str, exe: &str) -> String {
        format!(
            "<?xml version=\"1.0\"?>\n<Package>\n  <Identity Name=\"Dx.Test\" Version=\"1.0.0.0\" ProcessorArchitecture=\"{arch}\"/>\n  <Applications>\n    <Application Id=\"App\" Executable=\"{exe}\" EntryPoint=\"Windows.FullTrustApplication\">\n    </Application>\n  </Applications>\n</Package>\n"
        )
    }

    fn package(members: &[(&str, &[u8])]) -> tempfile::NamedTempFile {
        let mut zip = RawZip::new();
        for (name, data) in members {
            zip.add_file(name, data).expect("add member");
        }
        let file = tempfile::NamedTempFile::new().expect("tempfile");
        std::fs::write(file.path(), zip.finish().expect("finish zip")).expect("write zip");
        file
    }

    #[test]
    fn reads_arch_and_packed_exe() {
        let text = manifest("arm64", "hello.exe");
        let file = package(&[(MANIFEST, text.as_bytes()), ("hello.exe", b"MZstub")]);
        let package = read(file.path()).expect("read package");
        assert_eq!(package.arch, "arm64");
        assert_eq!(package.executable, "hello.exe");
        assert_eq!(package.exe, b"MZstub");
    }

    #[test]
    fn missing_manifest_is_named() {
        let file = package(&[("hello.exe", b"MZstub")]);
        let error = read(file.path()).expect_err("no manifest");
        assert!(error.contains("no member AppxManifest.xml"), "{error}");
    }

    #[test]
    fn missing_exe_is_named() {
        let text = manifest("x64", "absent.exe");
        let file = package(&[(MANIFEST, text.as_bytes())]);
        let error = read(file.path()).expect_err("no exe");
        assert!(error.contains("no member absent.exe"), "{error}");
    }

    #[test]
    fn backslash_executable_resolves_to_member() {
        let text = manifest("x64", "bin\\hello.exe");
        let file = package(&[(MANIFEST, text.as_bytes()), ("bin/hello.exe", b"MZbin")]);
        let package = read(file.path()).expect("read package");
        assert_eq!(package.executable, "bin/hello.exe");
        assert_eq!(package.exe, b"MZbin");
    }

    #[test]
    fn attribute_skips_applications_and_unescapes() {
        let xml = "<Applications><Application Id=\"A\" Executable=\"a&amp;b.exe\">";
        assert_eq!(
            attribute(xml, "<Application ", "Executable").as_deref(),
            Some("a&b.exe")
        );
        assert_eq!(attribute(xml, "<Application ", "Missing"), None);
    }
}
