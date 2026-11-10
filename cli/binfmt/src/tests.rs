use super::*;

fn put(image: &mut Vec<u8>, at: usize, bytes: &[u8]) {
    if image.len() < at + bytes.len() {
        image.resize(at + bytes.len(), 0);
    }
    image[at..at + bytes.len()].copy_from_slice(bytes);
}

fn elf(machine: u16, interp: Option<&str>, needed: &[&str]) -> Vec<u8> {
    elf_with(machine, interp, needed, true)
}

fn elf_with(machine: u16, interp: Option<&str>, needed: &[&str], sections: bool) -> Vec<u8> {
    let mut image = vec![0u8; 64];
    put(&mut image, 0, &[0x7f, b'E', b'L', b'F', 2, 1, 1, 0]);
    put(&mut image, 16, &2u16.to_le_bytes());
    put(&mut image, 18, &machine.to_le_bytes());
    put(&mut image, 20, &1u32.to_le_bytes());
    put(&mut image, 52, &64u16.to_le_bytes());
    put(&mut image, 54, &56u16.to_le_bytes());
    put(&mut image, 58, &64u16.to_le_bytes());
    let phnum = usize::from(interp.is_some()) + if needed.is_empty() { 0 } else { 2 };
    put(&mut image, 32, &64u64.to_le_bytes());
    put(&mut image, 56, &(phnum as u16).to_le_bytes());
    let mut next = 0usize;
    let mut header = |image: &mut Vec<u8>, kind: u32, at: u64, size: u64| {
        let base = 64 + next * 56;
        next += 1;
        put(image, base, &kind.to_le_bytes());
        put(image, base + 8, &at.to_le_bytes());
        put(image, base + 16, &at.to_le_bytes());
        put(image, base + 32, &size.to_le_bytes());
        put(image, base + 40, &size.to_le_bytes());
    };
    if let Some(path) = interp {
        let text = format!("{path}\0");
        header(&mut image, elf::PT_INTERP, 256, text.len() as u64);
        put(&mut image, 256, text.as_bytes());
    }
    if needed.is_empty() {
        return image;
    }
    let mut strings = vec![0u8];
    let mut offsets = Vec::new();
    for name in needed {
        offsets.push(strings.len() as u64);
        strings.extend_from_slice(name.as_bytes());
        strings.push(0);
    }
    let strings_at = 512usize;
    let dynamic_at = (strings_at + strings.len()).div_ceil(8) * 8;
    put(&mut image, strings_at, &strings);
    let mut dynamic = Vec::new();
    for offset in &offsets {
        dynamic.extend_from_slice(&u64::from(elf::DT_NEEDED).to_le_bytes());
        dynamic.extend_from_slice(&offset.to_le_bytes());
    }
    dynamic.extend_from_slice(&u64::from(elf::DT_STRTAB).to_le_bytes());
    dynamic.extend_from_slice(&(strings_at as u64).to_le_bytes());
    dynamic.extend_from_slice(&u64::from(elf::DT_STRSZ).to_le_bytes());
    dynamic.extend_from_slice(&(strings.len() as u64).to_le_bytes());
    dynamic.extend_from_slice(&[0u8; 16]);
    put(&mut image, dynamic_at, &dynamic);
    let end = (dynamic_at + dynamic.len()) as u64;
    header(&mut image, elf::PT_LOAD, 0, end);
    header(
        &mut image,
        elf::PT_DYNAMIC,
        dynamic_at as u64,
        dynamic.len() as u64,
    );
    if !sections {
        return image;
    }
    let headers_at = (dynamic_at + dynamic.len()).div_ceil(8) * 8;
    let mut section = |index: usize, kind: u32, at: usize, size: usize, link: u32, entsize: u64| {
        let base = headers_at + index * 64;
        put(&mut image, base + 4, &kind.to_le_bytes());
        put(&mut image, base + 24, &(at as u64).to_le_bytes());
        put(&mut image, base + 32, &(size as u64).to_le_bytes());
        put(&mut image, base + 40, &link.to_le_bytes());
        put(&mut image, base + 48, &8u64.to_le_bytes());
        put(&mut image, base + 56, &entsize.to_le_bytes());
    };
    section(0, 0, 0, 0, 0, 0);
    section(1, elf::SHT_STRTAB, strings_at, strings.len(), 0, 0);
    section(2, elf::SHT_DYNAMIC, dynamic_at, dynamic.len(), 1, 16);
    put(&mut image, 40, &(headers_at as u64).to_le_bytes());
    put(&mut image, 60, &3u16.to_le_bytes());
    put(&mut image, 62, &1u16.to_le_bytes());
    image
}

fn elf_comment(comment: Option<&[u8]>) -> Vec<u8> {
    let mut image = elf_with(62, None, &[], false);
    let Some(comment) = comment else {
        return image;
    };
    let names = b"\0.comment\0.shstrtab\0";
    put(&mut image, 128, comment);
    put(&mut image, 256, names);
    image.resize(512 + 3 * 64, 0);
    let mut section = |index: usize, name: u32, kind: u32, at: usize, size: usize| {
        let base = 512 + index * 64;
        put(&mut image, base, &name.to_le_bytes());
        put(&mut image, base + 4, &kind.to_le_bytes());
        put(&mut image, base + 24, &(at as u64).to_le_bytes());
        put(&mut image, base + 32, &(size as u64).to_le_bytes());
    };
    section(1, 1, elf::SHT_PROGBITS, 128, comment.len());
    section(2, 10, elf::SHT_STRTAB, 256, names.len());
    put(&mut image, 40, &512u64.to_le_bytes());
    put(&mut image, 58, &64u16.to_le_bytes());
    put(&mut image, 60, &3u16.to_le_bytes());
    put(&mut image, 62, &2u16.to_le_bytes());
    image
}

fn pe(machine: u16, subsystem: u16, dll: Option<&str>) -> Vec<u8> {
    let mut image = vec![0u8; 0x400];
    put(&mut image, 0, b"MZ");
    put(&mut image, 0x3c, &0x40u32.to_le_bytes());
    put(&mut image, 0x40, b"PE\0\0");
    let coff = 0x44;
    put(&mut image, coff, &machine.to_le_bytes());
    put(&mut image, coff + 2, &1u16.to_le_bytes());
    put(&mut image, coff + 16, &240u16.to_le_bytes());
    put(&mut image, coff + 18, &0x22u16.to_le_bytes());
    let optional = coff + 20;
    put(&mut image, optional, &0x20bu16.to_le_bytes());
    put(&mut image, optional + 32, &0x1000u32.to_le_bytes());
    put(&mut image, optional + 36, &0x200u32.to_le_bytes());
    put(&mut image, optional + 56, &0x2000u32.to_le_bytes());
    put(&mut image, optional + 60, &0x200u32.to_le_bytes());
    put(&mut image, optional + 68, &subsystem.to_le_bytes());
    put(&mut image, optional + 108, &16u32.to_le_bytes());
    let section = optional + 240;
    put(&mut image, section, b".idata\0\0");
    put(&mut image, section + 8, &0x200u32.to_le_bytes());
    put(&mut image, section + 12, &0x1000u32.to_le_bytes());
    put(&mut image, section + 16, &0x200u32.to_le_bytes());
    put(&mut image, section + 20, &0x200u32.to_le_bytes());
    put(&mut image, section + 36, &0xc000_0040u32.to_le_bytes());
    if let Some(name) = dll {
        put(&mut image, optional + 112 + 8, &0x1000u32.to_le_bytes());
        put(&mut image, optional + 112 + 12, &40u32.to_le_bytes());
        put(&mut image, 0x200 + 12, &0x1040u32.to_le_bytes());
        put(&mut image, 0x200 + 16, &0x1080u32.to_le_bytes());
        put(&mut image, 0x240, format!("{name}\0").as_bytes());
    }
    image
}

fn macho(cputype: u32, platform: u32, minos: u32, dylibs: &[&str]) -> Vec<u8> {
    let mut commands = Vec::new();
    commands.extend_from_slice(&macho::LC_BUILD_VERSION.to_le_bytes());
    commands.extend_from_slice(&24u32.to_le_bytes());
    commands.extend_from_slice(&platform.to_le_bytes());
    commands.extend_from_slice(&minos.to_le_bytes());
    commands.extend_from_slice(&minos.to_le_bytes());
    commands.extend_from_slice(&0u32.to_le_bytes());
    for name in dylibs {
        let size = (24 + name.len() + 1).div_ceil(8) * 8;
        let start = commands.len();
        commands.extend_from_slice(&macho::LC_LOAD_DYLIB.to_le_bytes());
        commands.extend_from_slice(&(size as u32).to_le_bytes());
        commands.extend_from_slice(&24u32.to_le_bytes());
        commands.extend_from_slice(&[0u8; 12]);
        commands.extend_from_slice(name.as_bytes());
        commands.resize(start + size, 0);
    }
    let mut image = Vec::new();
    image.extend_from_slice(&macho::MH_MAGIC_64.to_le_bytes());
    image.extend_from_slice(&cputype.to_le_bytes());
    image.extend_from_slice(&0u32.to_le_bytes());
    image.extend_from_slice(&macho::MH_EXECUTE.to_le_bytes());
    image.extend_from_slice(&(1 + dylibs.len() as u32).to_le_bytes());
    image.extend_from_slice(&(commands.len() as u32).to_le_bytes());
    image.extend_from_slice(&0u32.to_le_bytes());
    image.extend_from_slice(&0u32.to_le_bytes());
    image.extend_from_slice(&commands);
    image
}

fn fat(slices: &[Vec<u8>]) -> Vec<u8> {
    let mut image = Vec::new();
    image.extend_from_slice(&macho::FAT_MAGIC.to_be_bytes());
    image.extend_from_slice(&(slices.len() as u32).to_be_bytes());
    let mut at = 4096u32;
    for slice in slices {
        let cputype = u32::from_le_bytes([slice[4], slice[5], slice[6], slice[7]]);
        image.extend_from_slice(&cputype.to_be_bytes());
        image.extend_from_slice(&0u32.to_be_bytes());
        image.extend_from_slice(&at.to_be_bytes());
        image.extend_from_slice(&(slice.len() as u32).to_be_bytes());
        image.extend_from_slice(&12u32.to_be_bytes());
        at += 4096;
    }
    for (index, slice) in slices.iter().enumerate() {
        put(&mut image, 4096 * (index + 1), slice);
    }
    image
}

#[test]
fn elf_reads_machine_interpreter_and_needed() {
    let image = elf(
        elf::EM_X86_64,
        Some("/lib64/ld-linux-x86-64.so.2"),
        &["libm.so.6", "libc.so.6"],
    );
    assert_eq!(
        inspect(&image).unwrap(),
        vec![Image {
            format: Format::Elf,
            machine: "x86_64".to_owned(),
            abi: Abi::Gnu,
            needed: vec!["libc.so.6".to_owned(), "libm.so.6".to_owned()],
        }]
    );
}

#[test]
fn elf_names_musl_and_static_images() {
    let musl = inspect(&elf(
        elf::EM_AARCH64,
        Some("/lib/ld-musl-aarch64.so.1"),
        &["libc.musl-aarch64.so.1"],
    ))
    .unwrap();
    assert_eq!(musl[0].machine, "aarch64");
    assert_eq!(musl[0].abi, Abi::Musl);
    assert_eq!(musl[0].abi.name(), "musl");
    let fixed = inspect(&elf(elf::EM_X86_64, None, &[])).unwrap();
    assert_eq!(fixed[0].abi, Abi::Static);
    assert_eq!(fixed[0].abi.name(), "static");
    assert!(fixed[0].needed.is_empty());
}

#[test]
fn pe_reads_machine_subsystem_and_imports() {
    let image = inspect(&pe(pe::IMAGE_FILE_MACHINE_AMD64, 3, Some("KERNEL32.dll"))).unwrap();
    assert_eq!(
        image,
        vec![Image {
            format: Format::Pe,
            machine: "x86_64".to_owned(),
            abi: Abi::Msvc {
                subsystem: "console".to_owned()
            },
            needed: vec!["KERNEL32.dll".to_owned()],
        }]
    );
    assert_eq!(image[0].abi.name(), "msvc console");
    assert_eq!(image[0].format.name(), "pe");
    let gui = inspect(&pe(pe::IMAGE_FILE_MACHINE_ARM64, 2, None)).unwrap();
    assert_eq!(gui[0].machine, "aarch64");
    assert_eq!(gui[0].abi.name(), "msvc windows");
    assert!(gui[0].needed.is_empty());
    let odd = inspect(&pe(pe::IMAGE_FILE_MACHINE_AMD64, 10, None)).unwrap();
    assert_eq!(odd[0].abi.name(), "msvc subsystem-10");
}

#[test]
fn macho_reads_build_version_and_dylibs() {
    let image = macho(
        macho::CPU_TYPE_ARM64,
        macho::PLATFORM_MACOS,
        0x000b_0000,
        &["/usr/lib/libSystem.B.dylib"],
    );
    assert_eq!(machine(&image).as_deref(), Some("aarch64"));
    assert_eq!(
        inspect(&image).unwrap(),
        vec![Image {
            format: Format::MachO,
            machine: "aarch64".to_owned(),
            abi: Abi::Apple {
                platform: "macos".to_owned(),
                minos: "11.0".to_owned()
            },
            needed: vec!["/usr/lib/libSystem.B.dylib".to_owned()],
        }]
    );
    let patched = inspect(&macho(
        macho::CPU_TYPE_X86_64,
        macho::PLATFORM_IOS,
        0x000d_0201,
        &[],
    ))
    .unwrap();
    assert_eq!(patched[0].abi.name(), "ios 13.2.1");
    assert_eq!(patched[0].format.name(), "macho");
}

#[test]
fn fat_macho_yields_one_image_per_slice() {
    let images = inspect(&fat(&[
        macho(
            macho::CPU_TYPE_X86_64,
            macho::PLATFORM_MACOS,
            0x000a_0f00,
            &[],
        ),
        macho(
            macho::CPU_TYPE_ARM64,
            macho::PLATFORM_MACOS,
            0x000b_0000,
            &[],
        ),
    ]))
    .unwrap();
    let machines: Vec<&str> = images.iter().map(|image| image.machine.as_str()).collect();
    assert_eq!(machines, ["x86_64", "aarch64"]);
    assert_eq!(images[0].abi.name(), "macos 10.15");
}

#[test]
fn inspect_rejects_other_files() {
    assert!(inspect(b"#!/bin/sh\necho hi\n").is_err());
    assert!(inspect(&[]).is_err());
    let truncated = elf(elf::EM_X86_64, None, &["libc.so.6"]);
    assert!(inspect(&truncated[..100]).is_err());
}

#[test]
fn inspect_reads_the_running_test_binary() {
    let bytes = std::fs::read(std::env::current_exe().unwrap()).unwrap();
    let images = inspect(&bytes).unwrap();
    #[cfg(windows)]
    let want = Format::Pe;
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    let want = Format::MachO;
    #[cfg(not(any(windows, target_os = "macos", target_os = "ios")))]
    let want = Format::Elf;
    assert_eq!(images[0].format, want);
    assert!(
        images
            .iter()
            .any(|image| image.machine == std::env::consts::ARCH)
    );
}

#[test]
fn machine_reads_elf_header_bytes() {
    let mut x86 = vec![0x7f, b'E', b'L', b'F', 2, 1, 1, 0];
    x86.resize(18, 0);
    x86.extend_from_slice(&[0x3e, 0x00]);
    assert_eq!(machine(&x86).as_deref(), Some("x86_64"));
    let mut arm = x86.clone();
    arm[18] = 0xb7;
    assert_eq!(machine(&arm).as_deref(), Some("aarch64"));
    let mut big = x86.clone();
    big[5] = 2;
    big[18] = 0x00;
    big[19] = 0x28;
    assert_eq!(machine(&big).as_deref(), Some("arm"));
    let mut odd = x86.clone();
    odd[18] = 0x99;
    assert_eq!(machine(&odd).as_deref(), Some("elf-0x99"));
    let mut i386 = x86.clone();
    i386[18] = 0x03;
    assert_eq!(machine(&i386).as_deref(), Some("x86"));
    let mut riscv = x86.clone();
    riscv[18] = 0xf3;
    assert_eq!(machine(&riscv).as_deref(), Some("riscv"));
    let mut unknown_order = x86.clone();
    unknown_order[5] = 0;
    assert_eq!(machine(&unknown_order), None);
    assert_eq!(machine(b"!<arch>\nlibsolo.a padding"), None);
    assert_eq!(machine(&x86[..10]), None);
}

#[test]
fn machine_reads_the_coff_header() {
    let mut image = vec![0u8; 0x40];
    image[..2].copy_from_slice(b"MZ");
    image[0x3c] = 0x40;
    image.extend_from_slice(b"PE\0\0");
    image.extend_from_slice(&[0x64, 0x86]);
    assert_eq!(machine(&image).as_deref(), Some("x86_64"));
    let last = image.len() - 2;
    image[last] = 0x64;
    image[last + 1] = 0xaa;
    assert_eq!(machine(&image).as_deref(), Some("aarch64"));
    image[last] = 0x99;
    image[last + 1] = 0x00;
    assert_eq!(machine(&image).as_deref(), Some("pe-0x99"));
    image[0x40] = b'X';
    assert_eq!(machine(&image), None);
    assert_eq!(machine(&image[..0x20]), None);
    assert_eq!(machine(b"\x7fELF"), None);
}

#[test]
fn machine_reads_thin_macho_headers_in_both_orders() {
    let mut little = vec![0xcf, 0xfa, 0xed, 0xfe];
    little.extend_from_slice(&macho::CPU_TYPE_X86_64.to_le_bytes());
    assert_eq!(machine(&little).as_deref(), Some("x86_64"));
    let mut big = vec![0xfe, 0xed, 0xfa, 0xce];
    big.extend_from_slice(&macho::CPU_TYPE_ARM.to_be_bytes());
    assert_eq!(machine(&big).as_deref(), Some("arm"));
    let mut odd = vec![0xce, 0xfa, 0xed, 0xfe];
    odd.extend_from_slice(&7u32.to_le_bytes());
    assert_eq!(machine(&odd).as_deref(), Some("x86"));
    odd[4] = 0x55;
    assert_eq!(machine(&odd).as_deref(), Some("macho-0x55"));
}

#[test]
fn elf_reads_needed_without_section_headers() {
    let image = elf_with(
        elf::EM_X86_64,
        Some("/lib64/ld-linux-x86-64.so.2"),
        &["libm.so.6", "libc.so.6"],
        false,
    );
    let images = inspect(&image).unwrap();
    assert_eq!(images[0].needed, ["libc.so.6", "libm.so.6"]);
    assert_eq!(images[0].abi, Abi::Gnu);
}

#[test]
fn elf_rejects_dynamic_segment_without_string_table() {
    let mut image = elf_with(elf::EM_X86_64, None, &["libc.so.6"], false);
    // Turn DT_STRTAB (second entry after the one DT_NEEDED) into DT_DEBUG.
    let at = 512 + "\0libc.so.6\0".len();
    let dynamic_at = at.div_ceil(8) * 8;
    put(&mut image, dynamic_at + 16, &elf::DT_DEBUG.to_le_bytes());
    assert!(inspect(&image).is_err());
}

#[test]
fn macho_names_other_apple_platforms() {
    for (platform, name) in [
        (macho::PLATFORM_TVOS, "tvos"),
        (macho::PLATFORM_WATCHOS, "watchos"),
        (macho::PLATFORM_MACCATALYST, "maccatalyst"),
        (macho::PLATFORM_IOSSIMULATOR, "iossimulator"),
        (99, "platform-99"),
    ] {
        let image = inspect(&macho(macho::CPU_TYPE_ARM64, platform, 0x000e_0000, &[])).unwrap();
        assert_eq!(image[0].abi.name(), format!("{name} 14.0"));
    }
}

#[test]
fn macho_reads_version_min_macosx() {
    let mut image = macho(macho::CPU_TYPE_X86_64, macho::PLATFORM_MACOS, 0, &[]);
    // Replace the 24-byte LC_BUILD_VERSION with a 16-byte LC_VERSION_MIN_MACOSX.
    let mut commands = Vec::new();
    commands.extend_from_slice(&macho::LC_VERSION_MIN_MACOSX.to_le_bytes());
    commands.extend_from_slice(&16u32.to_le_bytes());
    commands.extend_from_slice(&0x000a_0e00u32.to_le_bytes());
    commands.extend_from_slice(&0u32.to_le_bytes());
    image.truncate(32);
    image.extend_from_slice(&commands);
    put(&mut image, 20, &16u32.to_le_bytes());
    let images = inspect(&image).unwrap();
    assert_eq!(images[0].abi.name(), "macos 10.14");
}

#[test]
fn macho_without_build_version_is_rejected() {
    let mut image = macho(macho::CPU_TYPE_X86_64, macho::PLATFORM_MACOS, 0, &[]);
    image.truncate(32);
    put(&mut image, 16, &0u32.to_le_bytes());
    put(&mut image, 20, &0u32.to_le_bytes());
    let error = inspect(&image).unwrap_err();
    assert_eq!(error.to_string(), "Mach-O image has no build version");
}

#[test]
fn fat_rejects_empty_and_non_macho_slices() {
    let mut empty = fat(&[]);
    empty.resize(32, 0);
    assert_eq!(
        inspect(&empty).unwrap_err().to_string(),
        "Mach-O fat binary has no slices"
    );
    let mut with_elf = fat(&[macho(
        macho::CPU_TYPE_X86_64,
        macho::PLATFORM_MACOS,
        0x000a_0f00,
        &[],
    )]);
    let slice = elf(elf::EM_X86_64, None, &[]);
    put(&mut with_elf, 4096, &slice);
    assert!(
        inspect(&with_elf)
            .unwrap_err()
            .to_string()
            .contains("fat slice")
    );
}

#[test]
fn error_display_and_format_names() {
    assert_eq!(Error("boom".to_owned()).to_string(), "boom");
    assert_eq!(Format::Elf.name(), "elf");
    assert_eq!(Abi::Gnu.name(), "gnu");
}

#[test]
fn inspect_rejects_unsupported_formats() {
    let error = inspect(b"!<arch>\n\0\0\0\0\0\0\0\0").unwrap_err();
    let text = error.to_string();
    assert!(text.contains("binary format"), "{text}");
}

fn coff(machine: u16) -> Vec<u8> {
    let mut object = vec![0u8; 20];
    put(&mut object, 0, &machine.to_le_bytes());
    object
}

fn import_object(machine: u16) -> Vec<u8> {
    let mut object = vec![0u8; 20];
    put(&mut object, 2, &0xffffu16.to_le_bytes());
    put(&mut object, 6, &machine.to_le_bytes());
    object
}

fn archive(members: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut out = b"!<arch>\n".to_vec();
    let linker = [("/", vec![0u8; 4]), ("/", vec![0u8; 4]), ("//", Vec::new())];
    for (name, data) in linker.iter().chain(members.iter()) {
        out.extend_from_slice(format!("{name:<16}{:<12}{:<6}{:<6}{:<8}", 0, 0, 0, 0).as_bytes());
        out.extend_from_slice(format!("{:<10}`\n", data.len()).as_bytes());
        out.extend_from_slice(data);
        if data.len() % 2 == 1 {
            out.push(b'\n');
        }
    }
    out
}

#[test]
fn machines_names_coff_objects() {
    assert_eq!(machines(&coff(0xaa64)).unwrap(), ["aarch64"]);
    assert_eq!(machines(&coff(0x8664)).unwrap(), ["x86_64"]);
}

#[test]
fn machines_names_import_objects() {
    assert_eq!(machines(&import_object(0xaa64)).unwrap(), ["aarch64"]);
}

#[test]
fn machines_names_every_archive_member() {
    let both = archive(&[("a.obj/", coff(0xaa64)), ("b.obj/", coff(0xaa64))]);
    assert_eq!(machines(&both).unwrap(), ["aarch64", "aarch64"]);
    let mixed = archive(&[("a.obj/", coff(0xaa64)), ("b.obj/", coff(0x8664))]);
    assert_eq!(machines(&mixed).unwrap(), ["aarch64", "x86_64"]);
    let mut odd = coff(0x8664);
    odd.push(0);
    assert_eq!(machines(&archive(&[("c.obj/", odd)])).unwrap(), ["x86_64"]);
    let import = archive(&[("a.dll/", import_object(0x8664))]);
    assert_eq!(machines(&import).unwrap(), ["x86_64"]);
}

#[test]
fn machines_rejects_an_empty_archive() {
    assert!(machines(&archive(&[])).is_err());
}

#[test]
fn machines_reads_executables() {
    assert_eq!(
        machines(&elf(elf::EM_AARCH64, None, &[])).unwrap(),
        ["aarch64"]
    );
}

#[test]
fn producers_split_the_elf_comment_section() {
    let image = elf_comment(Some(b"\0GCC: (GNU) 14.2.0\0Linker: LLD 19\0\0"));
    assert_eq!(producers(&image), ["GCC: (GNU) 14.2.0", "Linker: LLD 19"]);
}

#[test]
fn producers_are_empty_without_a_comment_or_for_other_formats() {
    assert!(producers(&elf_comment(None)).is_empty());
    assert!(producers(&pe(0x8664, 3, None)).is_empty());
    assert!(producers(&pe_with_rich(&[])).is_empty());
    assert!(producers(b"not a binary").is_empty());
}

fn pe_with_rich(entries: &[(u16, u16, u32)]) -> Vec<u8> {
    let key = 0x1234_5678u32;
    let mut image = pe(0x8664, 3, None);
    image.copy_within(0x40..0x1bc, 0x80);
    put(&mut image, 0x3c, &0x80u32.to_le_bytes());
    put(&mut image, 0x40, &(0x536e_6144 ^ key).to_le_bytes());
    for slot in 1..4 {
        put(&mut image, 0x40 + slot * 4, &key.to_le_bytes());
    }
    let mut at = 0x50;
    for (prodid, build, count) in entries {
        let id = (u32::from(*prodid) << 16) | u32::from(*build);
        put(&mut image, at, &(id ^ key).to_le_bytes());
        put(&mut image, at + 4, &(count ^ key).to_le_bytes());
        at += 8;
    }
    put(&mut image, at, b"Rich");
    put(&mut image, at + 4, &key.to_le_bytes());
    image
}

#[test]
fn producers_decode_the_pe_rich_header() {
    let image = pe_with_rich(&[(261, 35717, 3), (259, 35717, 1), (261, 35717, 2), (1, 0, 0)]);
    assert_eq!(producers(&image), ["msvc 261.35717", "msvc 259.35717"]);
    assert_eq!(machines(&image).unwrap(), ["x86_64"]);
}

#[test]
fn pe_machine_name_names_every_machine() {
    assert_eq!(pe_machine_name(pe::IMAGE_FILE_MACHINE_I386), "x86");
    assert_eq!(pe_machine_name(pe::IMAGE_FILE_MACHINE_ARMNT), "arm");
    assert_eq!(pe_machine_name(0x1234), "pe-0x1234");
}

#[test]
fn inspect_accepts_32_bit_headers() {
    let mut elf32 = vec![0u8; 52];
    put(&mut elf32, 0, &[0x7f, b'E', b'L', b'F', 1, 1, 1, 0]);
    put(&mut elf32, 16, &2u16.to_le_bytes());
    put(&mut elf32, 18, &3u16.to_le_bytes());
    put(&mut elf32, 20, &1u32.to_le_bytes());
    put(&mut elf32, 40, &52u16.to_le_bytes());
    assert_eq!(inspect(&elf32).unwrap()[0].machine, "x86");
    assert!(producers(&elf32).is_empty());

    let mut thin = macho(
        macho::CPU_TYPE_ARM64,
        macho::PLATFORM_MACOS,
        0x000b_0000,
        &[],
    );
    put(&mut thin, 0, &macho::MH_MAGIC.to_le_bytes());
    thin.drain(28..32);
    assert_eq!(inspect(&thin).unwrap()[0].machine, "aarch64");
    assert_eq!(
        inspect(&fat(&[thin.clone()])).unwrap()[0].machine,
        "aarch64"
    );

    let mut pe32 = pe(0x14c, 3, None);
    put(&mut pe32, 0x44 + 20, &0x10bu16.to_le_bytes());
    assert!(inspect(&pe32).is_ok() || inspect(&pe32).is_err());
}

#[test]
fn inspect_reads_a_64_bit_fat_header() {
    let slice = macho(
        macho::CPU_TYPE_ARM64,
        macho::PLATFORM_MACOS,
        0x000b_0000,
        &[],
    );
    let mut image = Vec::new();
    image.extend_from_slice(&macho::FAT_MAGIC_64.to_be_bytes());
    image.extend_from_slice(&1u32.to_be_bytes());
    image.extend_from_slice(&macho::CPU_TYPE_ARM64.to_be_bytes());
    image.extend_from_slice(&0u32.to_be_bytes());
    image.extend_from_slice(&4096u64.to_be_bytes());
    image.extend_from_slice(&(slice.len() as u64).to_be_bytes());
    image.extend_from_slice(&12u32.to_be_bytes());
    image.extend_from_slice(&0u32.to_be_bytes());
    image.resize(4096, 0);
    image.extend_from_slice(&slice);
    assert_eq!(inspect(&image).unwrap()[0].machine, "aarch64");
}

#[test]
fn machines_reads_a_big_object_header() {
    let mut object = vec![0u8; 56];
    put(&mut object, 2, &0xffffu16.to_le_bytes());
    put(&mut object, 4, &2u16.to_le_bytes());
    put(&mut object, 6, &0x8664u16.to_le_bytes());
    put(
        &mut object,
        12,
        object::pod::bytes_of(&pe::ANON_OBJECT_HEADER_BIGOBJ_CLASS_ID),
    );
    assert_eq!(machines(&object).unwrap(), ["x86_64"]);
}

#[test]
fn machines_rejects_a_nested_or_cut_archive() {
    let nested = archive(&[("a.a/", archive(&[]))]);
    assert!(machines(&nested).is_err());
    let mut cut = archive(&[("a.obj/", coff(0xaa64))]);
    cut.truncate(cut.len() - 8);
    assert!(machines(&cut).is_err());
}

#[test]
fn elf32_with_bad_program_headers_is_rejected() {
    let mut elf32 = vec![0u8; 52];
    put(&mut elf32, 0, &[0x7f, b'E', b'L', b'F', 1, 1, 1, 0]);
    put(&mut elf32, 28, &60u32.to_le_bytes());
    put(&mut elf32, 40, &52u16.to_le_bytes());
    put(&mut elf32, 42, &32u16.to_le_bytes());
    put(&mut elf32, 44, &1u16.to_le_bytes());
    assert!(inspect(&elf32).is_err());
}

#[test]
fn elf_dynamic_segment_without_needed_entries_names_none() {
    let mut image = elf_with(elf::EM_X86_64, None, &["libc.so.6"], false);
    put(&mut image, 528, &u64::from(elf::DT_SONAME).to_le_bytes());
    assert!(inspect(&image).unwrap()[0].needed.is_empty());
}

#[test]
fn macho_ignores_load_commands_it_does_not_read() {
    let mut image = macho(
        macho::CPU_TYPE_ARM64,
        macho::PLATFORM_MACOS,
        0x000b_0000,
        &[],
    );
    image.extend_from_slice(&macho::LC_SYMTAB.to_le_bytes());
    image.extend_from_slice(&24u32.to_le_bytes());
    image.extend_from_slice(&[0u8; 16]);
    put(&mut image, 16, &2u32.to_le_bytes());
    put(&mut image, 20, &48u32.to_le_bytes());
    assert_eq!(inspect(&image).unwrap()[0].machine, "aarch64");
}

#[test]
fn producers_are_empty_for_a_pe_without_a_dans_marker() {
    let mut image = pe_with_rich(&[(1, 2, 3)]);
    put(&mut image, 0x40, &0u32.to_le_bytes());
    assert!(producers(&image).is_empty());
}
