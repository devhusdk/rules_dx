//! Import closure: which libraries a shipped binary may import without shipping them.

/// System libraries a shipped binary may import without shipping them; a trailing '*' is a prefix.
pub const SYSTEM_IMPORTS: [(&str, &[&str]); 3] = [
    (
        "linux",
        &[
            "ld-linux-aarch64.so.1",
            "ld-linux-x86-64.so.2",
            "libc.so.6",
            "libdl.so.2",
            "libgcc_s.so.1",
            "libm.so.6",
            "libpthread.so.0",
            "librt.so.1",
        ],
    ),
    (
        "macos",
        &[
            "/System/Library/Frameworks/*",
            "/usr/lib/libSystem.B.dylib",
            "/usr/lib/libc++.1.dylib",
            "/usr/lib/libiconv.2.dylib",
            "/usr/lib/libobjc.A.dylib",
            "/usr/lib/libresolv.9.dylib",
        ],
    ),
    (
        "windows",
        &[
            "advapi32.dll",
            "api-ms-win-*",
            "authz.dll",
            "bcrypt.dll",
            "bcryptprimitives.dll",
            "crypt32.dll",
            "dbghelp.dll",
            "dnsapi.dll",
            "icuuc.dll",
            "iphlpapi.dll",
            "kernel32.dll",
            "mpr.dll",
            "ncrypt.dll",
            "netapi32.dll",
            "ntdll.dll",
            "ole32.dll",
            "oleaut32.dll",
            "psapi.dll",
            "secur32.dll",
            "shell32.dll",
            "synchronization.dll",
            "ucrtbase.dll",
            "user32.dll",
            "userenv.dll",
            "version.dll",
            "winhttp.dll",
            "winmm.dll",
            "ws2_32.dll",
        ],
    ),
];

/// Returns whether `needed` is a system import on `os` or one of the shipped runtime members.
pub fn import_allowed(os: &str, needed: &str, runtime: &[String]) -> bool {
    let fold = |text: &str| {
        if os == "windows" {
            text.to_ascii_lowercase()
        } else {
            text.to_owned()
        }
    };
    let needed = fold(needed);
    let base = needed.rsplit('/').next().unwrap_or_default();
    if runtime.iter().any(|member| fold(member) == base) {
        return true;
    }
    SYSTEM_IMPORTS
        .iter()
        .filter(|(system, _)| *system == os)
        .flat_map(|(_, names)| names.iter())
        .any(|pattern| match pattern.strip_suffix('*') {
            Some(prefix) => needed.starts_with(prefix),
            None => needed == *pattern,
        })
}

fn os_for(format: &str) -> &'static str {
    match format {
        "elf" => "linux",
        "macho" => "macos",
        _ => "windows",
    }
}

fn matches_pattern(os: &str, needed: &str, pattern: &str) -> bool {
    let (needed, pattern) = if os == "windows" {
        (needed.to_ascii_lowercase(), pattern.to_ascii_lowercase())
    } else {
        (needed.to_owned(), pattern.to_owned())
    };
    match pattern.strip_suffix('*') {
        Some(prefix) => needed.starts_with(prefix),
        None => needed == pattern,
    }
}

/// Returns the imports of `image_bytes` that are neither shipped `members`, system imports,
/// nor named by `provides`; bytes that are not a binary image import nothing.
pub fn missing_imports(
    image_bytes: &[u8],
    members: &[String],
    provides: &[String],
) -> Result<Vec<String>, String> {
    let Ok(images) = dx_binfmt::inspect(image_bytes) else {
        return Ok(Vec::new());
    };
    let mut missing: Vec<String> = Vec::new();
    for image in images {
        let os = os_for(image.format.name());
        for needed in &image.needed {
            let base = needed.rsplit('/').next().unwrap_or_default();
            if !import_allowed(os, needed, members)
                && !provides.iter().any(|name| {
                    matches_pattern(os, needed, name) || matches_pattern(os, base, name)
                })
                && !missing.contains(needed)
            {
                missing.push(needed.clone());
            }
        }
    }
    Ok(missing)
}

/// Runs `[--check <member>]... [--provides <name>]... <member=path>...`: every checked member
/// (all members when none is named) must import only shipped, system or provided libraries.
pub fn check_imports(args: &[String]) -> Result<(), String> {
    let mut checks: Vec<&str> = Vec::new();
    let mut provides: Vec<String> = Vec::new();
    let mut rest = args;
    while let [flag, value, after @ ..] = rest {
        match flag.as_str() {
            "--check" => checks.push(value),
            "--provides" => provides.push(value.clone()),
            _ => break,
        }
        rest = after;
    }
    let mut members: Vec<(&str, &str)> = Vec::new();
    for pair in rest {
        let (name, path) = pair
            .split_once('=')
            .ok_or_else(|| format!("member {pair:?} must be <name=path>"))?;
        members.push((name, path));
    }
    if members.is_empty() {
        return Err("need at least one <member=path>".to_owned());
    }
    let shipped: Vec<String> = members
        .iter()
        .map(|(name, _)| name.rsplit('/').next().unwrap_or_default().to_owned())
        .collect();
    let mut problems = Vec::new();
    for (name, path) in &members {
        if !checks.is_empty() && !checks.contains(name) {
            continue;
        }
        let bytes = std::fs::read(path).map_err(|error| format!("cannot read {path}: {error}"))?;
        let missing = missing_imports(&bytes, &shipped, &provides)?;
        if !missing.is_empty() {
            problems.push(format!(
                "{name}: not shipped and not a system import: {missing:?}"
            ));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(image: &mut Vec<u8>, at: usize, bytes: &[u8]) {
        if image.len() < at + bytes.len() {
            image.resize(at + bytes.len(), 0);
        }
        image[at..at + bytes.len()].copy_from_slice(bytes);
    }

    /// Builds one minimal 64-bit little-endian ELF executable with DT_NEEDED entries.
    fn elf(machine: u16, needed: &[&str]) -> Vec<u8> {
        let mut image = vec![0u8; 64];
        put(&mut image, 0, &[0x7f, b'E', b'L', b'F', 2, 1, 1, 0]);
        put(&mut image, 16, &2u16.to_le_bytes());
        put(&mut image, 18, &machine.to_le_bytes());
        put(&mut image, 20, &1u32.to_le_bytes());
        put(&mut image, 52, &64u16.to_le_bytes());
        put(&mut image, 54, &56u16.to_le_bytes());
        put(&mut image, 58, &64u16.to_le_bytes());
        let mut strings = vec![0u8];
        let mut offsets = Vec::new();
        for name in needed {
            offsets.push(strings.len() as u64);
            strings.extend_from_slice(name.as_bytes());
            strings.push(0);
        }
        let strings_at = 256usize;
        let dynamic_at = (strings_at + strings.len()).div_ceil(8) * 8;
        put(&mut image, strings_at, &strings);
        let mut dynamic = Vec::new();
        for offset in &offsets {
            dynamic.extend_from_slice(&1u64.to_le_bytes());
            dynamic.extend_from_slice(&offset.to_le_bytes());
        }
        for (tag, value) in [(5u64, strings_at as u64), (10, strings.len() as u64)] {
            dynamic.extend_from_slice(&tag.to_le_bytes());
            dynamic.extend_from_slice(&value.to_le_bytes());
        }
        dynamic.extend_from_slice(&[0u8; 16]);
        put(&mut image, dynamic_at, &dynamic);
        let file_end = (dynamic_at + dynamic.len()) as u64;
        put(&mut image, 32, &64u64.to_le_bytes());
        put(&mut image, 56, &2u16.to_le_bytes());
        for (index, (kind, at, size)) in [
            (1u32, 0u64, file_end),
            (2, dynamic_at as u64, dynamic.len() as u64),
        ]
        .into_iter()
        .enumerate()
        {
            let base = 64 + index * 56;
            put(&mut image, base, &kind.to_le_bytes());
            put(&mut image, base + 8, &at.to_le_bytes());
            put(&mut image, base + 16, &at.to_le_bytes());
            put(&mut image, base + 32, &size.to_le_bytes());
            put(&mut image, base + 40, &size.to_le_bytes());
        }
        let headers_at = (dynamic_at + dynamic.len()).div_ceil(8) * 8;
        let sections = [
            (0u32, 0usize, 0usize, 0u32, 0u64),
            (3, strings_at, strings.len(), 0, 0),
            (6, dynamic_at, dynamic.len(), 1, 16),
        ];
        for (index, (kind, at, size, link, entsize)) in sections.into_iter().enumerate() {
            let base = headers_at + index * 64;
            put(&mut image, base + 4, &kind.to_le_bytes());
            put(&mut image, base + 24, &(at as u64).to_le_bytes());
            put(&mut image, base + 32, &(size as u64).to_le_bytes());
            put(&mut image, base + 40, &link.to_le_bytes());
            put(&mut image, base + 48, &8u64.to_le_bytes());
            put(&mut image, base + 56, &entsize.to_le_bytes());
        }
        put(&mut image, 40, &(headers_at as u64).to_le_bytes());
        put(&mut image, 60, &3u16.to_le_bytes());
        put(&mut image, 62, &1u16.to_le_bytes());
        image
    }

    #[test]
    fn import_allowed_matches_system_lists_runtime_and_windows_case() {
        let runtime = vec!["VCRUNTIME140.dll".to_owned()];
        assert!(import_allowed("windows", "KERNEL32.dll", &[]));
        assert!(import_allowed(
            "windows",
            "api-ms-win-crt-runtime-l1-1-0.dll",
            &[]
        ));
        assert!(import_allowed("windows", "vcruntime140.dll", &runtime));
        assert!(!import_allowed("windows", "vcruntime140.dll", &[]));
        assert!(import_allowed("macos", "/usr/lib/libSystem.B.dylib", &[]));
        assert!(import_allowed(
            "macos",
            "/System/Library/Frameworks/Security.framework/Versions/A/Security",
            &[]
        ));
        assert!(import_allowed(
            "macos",
            "@rpath/libfoo.dylib",
            &["libfoo.dylib".to_owned()]
        ));
        assert!(!import_allowed(
            "macos",
            "/opt/homebrew/lib/libfoo.dylib",
            &[]
        ));
        assert!(import_allowed("linux", "libc.so.6", &[]));
        assert!(import_allowed("linux", "libgcc_s.so.1", &[]));
        assert!(!import_allowed("linux", "LIBC.so.6", &[]));
        assert!(!import_allowed("linux", "libssl.so.3", &[]));
        assert!(!import_allowed("solaris", "libc.so.6", &[]));
    }

    fn strings(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn missing_imports_reports_unshipped_and_honours_provides() {
        let image = elf(62, &["libc.so.6", "libstdc++.so.6", "libqt.so.6"]);
        assert_eq!(
            missing_imports(&image, &[], &[]).unwrap(),
            strings(&["libqt.so.6", "libstdc++.so.6"])
        );
        assert_eq!(
            missing_imports(&image, &strings(&["libstdc++.so.6"]), &strings(&["libqt*"])).unwrap(),
            Vec::<String>::new()
        );
    }

    fn pe_importing(dll: &str) -> Vec<u8> {
        let mut image = vec![0u8; 0x400];
        put(&mut image, 0, b"MZ");
        put(&mut image, 0x3c, &0x40u32.to_le_bytes());
        put(&mut image, 0x40, b"PE\0\0");
        let coff = 0x44;
        put(&mut image, coff, &0x8664u16.to_le_bytes());
        put(&mut image, coff + 2, &1u16.to_le_bytes());
        put(&mut image, coff + 16, &240u16.to_le_bytes());
        put(&mut image, coff + 18, &0x22u16.to_le_bytes());
        let optional = coff + 20;
        put(&mut image, optional, &0x20bu16.to_le_bytes());
        put(&mut image, optional + 32, &0x1000u32.to_le_bytes());
        put(&mut image, optional + 36, &0x200u32.to_le_bytes());
        put(&mut image, optional + 56, &0x2000u32.to_le_bytes());
        put(&mut image, optional + 60, &0x200u32.to_le_bytes());
        put(&mut image, optional + 68, &3u16.to_le_bytes());
        put(&mut image, optional + 108, &16u32.to_le_bytes());
        let section = optional + 240;
        put(&mut image, section, b".idata\0\0");
        put(&mut image, section + 8, &0x200u32.to_le_bytes());
        put(&mut image, section + 12, &0x1000u32.to_le_bytes());
        put(&mut image, section + 16, &0x200u32.to_le_bytes());
        put(&mut image, section + 20, &0x200u32.to_le_bytes());
        put(&mut image, section + 36, &0xc000_0040u32.to_le_bytes());
        put(&mut image, optional + 112 + 8, &0x1000u32.to_le_bytes());
        put(&mut image, optional + 112 + 12, &40u32.to_le_bytes());
        put(&mut image, 0x200 + 12, &0x1040u32.to_le_bytes());
        put(&mut image, 0x200 + 16, &0x1080u32.to_le_bytes());
        put(&mut image, 0x240, format!("{dll}\0").as_bytes());
        image
    }

    #[test]
    fn windows_app_needs_vcruntime_shipped_but_not_the_ucrt() {
        let app = pe_importing("VCRUNTIME140.dll");
        assert_eq!(
            missing_imports(&app, &strings(&["app.exe"]), &[]).unwrap(),
            strings(&["VCRUNTIME140.dll"])
        );
        let shipped = strings(&["app.exe", "vcruntime140.dll", "msvcp140.dll"]);
        assert!(missing_imports(&app, &shipped, &[]).unwrap().is_empty());
        let ucrt = pe_importing("api-ms-win-crt-runtime-l1-1-0.dll");
        assert!(missing_imports(&ucrt, &[], &[]).unwrap().is_empty());
    }

    #[test]
    fn missing_imports_skips_non_images() {
        assert!(missing_imports(b"not a library", &[], &[])
            .unwrap()
            .is_empty());
    }
}
