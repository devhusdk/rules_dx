#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

//! Reads the format, machine, ABI and imported libraries of an executable.

use std::fmt;

use object::elf;
use object::macho;
use object::pe;
use object::read::elf::{Dyn as _, ProgramHeader as _};
use object::read::macho::{FatArch, LoadCommandVariant, MachHeader};
use object::read::pe::{ImageNtHeaders, ImageOptionalHeader as _, PeFile};
use object::{FileKind, LittleEndian};

/// One reason a binary could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

fn fail(what: &str, error: impl fmt::Display) -> Error {
    Error(format!("{what}: {error}"))
}

/// The container format of one binary image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Elf,
    MachO,
    Pe,
}

impl Format {
    /// Returns the lowercase format name.
    pub fn name(self) -> &'static str {
        match self {
            Format::Elf => "elf",
            Format::MachO => "macho",
            Format::Pe => "pe",
        }
    }
}

/// The runtime ABI one image needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Abi {
    Gnu,
    Musl,
    Static,
    Apple { platform: String, minos: String },
    Msvc { subsystem: String },
}

impl Abi {
    /// Returns the ABI as one short word or phrase.
    pub fn name(&self) -> String {
        match self {
            Abi::Gnu => "gnu".to_owned(),
            Abi::Musl => "musl".to_owned(),
            Abi::Static => "static".to_owned(),
            Abi::Apple { platform, minos } => format!("{platform} {minos}"),
            Abi::Msvc { subsystem } => format!("msvc {subsystem}"),
        }
    }
}

/// What one binary image says about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub format: Format,
    pub machine: String,
    pub abi: Abi,
    pub needed: Vec<String>,
}

/// Names an ELF e_machine value.
pub fn elf_machine_name(raw: u16) -> String {
    match raw {
        elf::EM_386 => "x86".to_owned(),
        elf::EM_ARM => "arm".to_owned(),
        elf::EM_X86_64 => "x86_64".to_owned(),
        elf::EM_AARCH64 => "aarch64".to_owned(),
        elf::EM_RISCV => "riscv".to_owned(),
        _ => format!("elf-0x{raw:x}"),
    }
}

/// Names a PE COFF machine value.
pub fn pe_machine_name(raw: u16) -> String {
    match raw {
        pe::IMAGE_FILE_MACHINE_I386 => "x86".to_owned(),
        pe::IMAGE_FILE_MACHINE_AMD64 => "x86_64".to_owned(),
        pe::IMAGE_FILE_MACHINE_ARM64 => "aarch64".to_owned(),
        pe::IMAGE_FILE_MACHINE_ARMNT => "arm".to_owned(),
        _ => format!("pe-0x{raw:x}"),
    }
}

/// Names a Mach-O cputype value.
pub fn macho_machine_name(raw: u32) -> String {
    match raw {
        macho::CPU_TYPE_X86 => "x86".to_owned(),
        macho::CPU_TYPE_X86_64 => "x86_64".to_owned(),
        macho::CPU_TYPE_ARM => "arm".to_owned(),
        macho::CPU_TYPE_ARM64 => "aarch64".to_owned(),
        _ => format!("macho-0x{raw:x}"),
    }
}

/// Names the machine from the first header bytes of an ELF, PE or thin Mach-O file.
pub fn machine(head: &[u8]) -> Option<String> {
    if head.len() >= 20 && head.starts_with(b"\x7fELF") {
        let raw = match head[5] {
            1 => u16::from_le_bytes([head[18], head[19]]),
            2 => u16::from_be_bytes([head[18], head[19]]),
            _ => return None,
        };
        return Some(elf_machine_name(raw));
    }
    if head.len() >= 0x40 && head.starts_with(b"MZ") {
        let offset = u32::from_le_bytes([head[0x3c], head[0x3d], head[0x3e], head[0x3f]]) as usize;
        let header = head.get(offset..offset.checked_add(6)?)?;
        if &header[..4] != b"PE\0\0" {
            return None;
        }
        return Some(pe_machine_name(u16::from_le_bytes([header[4], header[5]])));
    }
    if head.len() >= 8 {
        let magic = [head[0], head[1], head[2], head[3]];
        let cpu = [head[4], head[5], head[6], head[7]];
        return match magic {
            [0xcf, 0xfa, 0xed, 0xfe] | [0xce, 0xfa, 0xed, 0xfe] => {
                Some(macho_machine_name(u32::from_le_bytes(cpu)))
            }
            [0xfe, 0xed, 0xfa, 0xcf] | [0xfe, 0xed, 0xfa, 0xce] => {
                Some(macho_machine_name(u32::from_be_bytes(cpu)))
            }
            _ => None,
        };
    }
    None
}

/// Reads every image in one binary; a fat Mach-O yields one image per slice.
pub fn inspect(bytes: &[u8]) -> Result<Vec<Image>, Error> {
    let kind = FileKind::parse(bytes).map_err(|error| fail("unknown binary format", error))?;
    match kind {
        FileKind::Elf32 => Ok(vec![inspect_elf::<elf::FileHeader32<object::Endianness>>(
            bytes,
        )?]),
        FileKind::Elf64 => Ok(vec![inspect_elf::<elf::FileHeader64<object::Endianness>>(
            bytes,
        )?]),
        FileKind::MachO32 => Ok(vec![inspect_macho::<
            macho::MachHeader32<object::Endianness>,
        >(bytes)?]),
        FileKind::MachO64 => Ok(vec![inspect_macho::<
            macho::MachHeader64<object::Endianness>,
        >(bytes)?]),
        FileKind::MachOFat32 => inspect_fat::<macho::FatArch32>(bytes),
        FileKind::MachOFat64 => inspect_fat::<macho::FatArch64>(bytes),
        FileKind::Pe32 => Ok(vec![inspect_pe::<pe::ImageNtHeaders32>(bytes)?]),
        FileKind::Pe64 => Ok(vec![inspect_pe::<pe::ImageNtHeaders64>(bytes)?]),
        other => Err(Error(format!("unsupported binary format {other:?}"))),
    }
}

fn utf8(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names.dedup();
    names
}

fn inspect_elf<H: object::read::elf::FileHeader<Endian = object::Endianness>>(
    bytes: &[u8],
) -> Result<Image, Error> {
    let header = H::parse(bytes).map_err(|error| fail("bad ELF header", error))?;
    let endian = header
        .endian()
        .map_err(|error| fail("bad ELF header", error))?;
    let mut interp = None;
    for segment in header
        .program_headers(endian, bytes)
        .map_err(|error| fail("bad ELF program headers", error))?
    {
        if let Some(found) = segment
            .interpreter(endian, bytes)
            .map_err(|error| fail("bad ELF interpreter", error))?
        {
            interp = Some(utf8(found));
        }
    }
    let abi = match interp {
        None => Abi::Static,
        Some(path) if path.contains("musl") => Abi::Musl,
        Some(_) => Abi::Gnu,
    };
    let segments = header
        .program_headers(endian, bytes)
        .map_err(|error| fail("bad ELF program headers", error))?;
    let mut needed = Vec::new();
    for segment in segments {
        let Some(entries) = segment
            .dynamic(endian, bytes)
            .map_err(|error| fail("bad ELF dynamic segment", error))?
        else {
            continue;
        };
        let (mut strtab, mut strsz) = (None, 0u64);
        for entry in entries {
            let tag: u64 = entry.d_tag(endian).into();
            if tag == u64::from(elf::DT_NULL) {
                break;
            } else if tag == u64::from(elf::DT_STRTAB) {
                strtab = Some(entry.d_val(endian).into());
            } else if tag == u64::from(elf::DT_STRSZ) {
                strsz = entry.d_val(endian).into();
            }
        }
        let mut wanted = entries
            .iter()
            .take_while(|entry| entry.tag32(endian) != Some(elf::DT_NULL))
            .filter(|entry| entry.tag32(endian) == Some(elf::DT_NEEDED))
            .peekable();
        if wanted.peek().is_none() {
            continue;
        }
        let address =
            strtab.ok_or_else(|| Error("ELF dynamic segment has no string table".to_owned()))?;
        let table = segments
            .iter()
            .find_map(|load| {
                (load.p_type(endian) == elf::PT_LOAD)
                    .then(|| {
                        load.data_range(endian, bytes, address, strsz)
                            .ok()
                            .flatten()
                    })
                    .flatten()
            })
            .ok_or_else(|| Error("ELF dynamic string table is outside the file".to_owned()))?;
        let strings = object::read::StringTable::new(table, 0, table.len() as u64);
        for entry in wanted {
            let name = entry
                .string(endian, strings)
                .map_err(|error| fail("bad ELF needed entry", error))?;
            needed.push(utf8(name));
        }
    }
    Ok(Image {
        format: Format::Elf,
        machine: elf_machine_name(header.e_machine(endian)),
        abi,
        needed: sorted(needed),
    })
}

fn apple_platform(raw: u32) -> String {
    match raw {
        macho::PLATFORM_MACOS => "macos".to_owned(),
        macho::PLATFORM_IOS => "ios".to_owned(),
        macho::PLATFORM_TVOS => "tvos".to_owned(),
        macho::PLATFORM_WATCHOS => "watchos".to_owned(),
        macho::PLATFORM_MACCATALYST => "maccatalyst".to_owned(),
        macho::PLATFORM_IOSSIMULATOR => "iossimulator".to_owned(),
        _ => format!("platform-{raw}"),
    }
}

fn apple_version(raw: u32) -> String {
    let (major, minor, patch) = (raw >> 16, (raw >> 8) & 0xff, raw & 0xff);
    if patch == 0 {
        format!("{major}.{minor}")
    } else {
        format!("{major}.{minor}.{patch}")
    }
}

fn inspect_macho<H: MachHeader<Endian = object::Endianness>>(bytes: &[u8]) -> Result<Image, Error> {
    let header = H::parse(bytes, 0).map_err(|error| fail("bad Mach-O header", error))?;
    let endian = header
        .endian()
        .map_err(|error| fail("bad Mach-O header", error))?;
    let mut commands = header
        .load_commands(endian, bytes, 0)
        .map_err(|error| fail("bad Mach-O load commands", error))?;
    let mut abi = None;
    let mut needed = Vec::new();
    while let Some(command) = commands
        .next()
        .map_err(|error| fail("bad Mach-O load command", error))?
    {
        match command
            .variant()
            .map_err(|error| fail("bad Mach-O load command", error))?
        {
            LoadCommandVariant::Dylib(dylib) => {
                let name = command
                    .string(endian, dylib.dylib.name)
                    .map_err(|error| fail("bad Mach-O dylib name", error))?;
                needed.push(utf8(name));
            }
            LoadCommandVariant::BuildVersion(build) => {
                abi = Some(Abi::Apple {
                    platform: apple_platform(build.platform.get(endian)),
                    minos: apple_version(build.minos.get(endian)),
                });
            }
            LoadCommandVariant::VersionMin(min)
                if abi.is_none() && command.cmd() == macho::LC_VERSION_MIN_MACOSX =>
            {
                abi = Some(Abi::Apple {
                    platform: "macos".to_owned(),
                    minos: apple_version(min.version.get(endian)),
                });
            }
            _ => {}
        }
    }
    Ok(Image {
        format: Format::MachO,
        machine: macho_machine_name(header.cputype(endian)),
        abi: abi.ok_or_else(|| Error("Mach-O image has no build version".to_owned()))?,
        needed: sorted(needed),
    })
}

fn inspect_fat<A: FatArch>(bytes: &[u8]) -> Result<Vec<Image>, Error> {
    let fat = object::read::macho::MachOFatFile::<A>::parse(bytes)
        .map_err(|error| fail("bad Mach-O fat header", error))?;
    let mut images = Vec::new();
    for arch in fat.arches() {
        let slice = arch
            .data(bytes)
            .map_err(|error| fail("bad Mach-O fat slice", error))?;
        match FileKind::parse(slice).map_err(|error| fail("bad Mach-O fat slice", error))? {
            FileKind::MachO32 => images.push(inspect_macho::<
                macho::MachHeader32<object::Endianness>,
            >(slice)?),
            FileKind::MachO64 => images.push(inspect_macho::<
                macho::MachHeader64<object::Endianness>,
            >(slice)?),
            other => return Err(Error(format!("Mach-O fat slice is {other:?}"))),
        }
    }
    if images.is_empty() {
        return Err(Error("Mach-O fat binary has no slices".to_owned()));
    }
    Ok(images)
}

fn pe_subsystem(raw: u16) -> String {
    match raw {
        pe::IMAGE_SUBSYSTEM_WINDOWS_CUI => "console".to_owned(),
        pe::IMAGE_SUBSYSTEM_WINDOWS_GUI => "windows".to_owned(),
        _ => format!("subsystem-{raw}"),
    }
}

fn inspect_pe<Pe: ImageNtHeaders>(bytes: &[u8]) -> Result<Image, Error> {
    let file = PeFile::<Pe>::parse(bytes).map_err(|error| fail("bad PE image", error))?;
    let headers = file.nt_headers();
    let mut needed = Vec::new();
    if let Some(table) = file
        .import_table()
        .map_err(|error| fail("bad PE import table", error))?
    {
        let mut descriptors = table
            .descriptors()
            .map_err(|error| fail("bad PE import table", error))?;
        while let Some(descriptor) = descriptors
            .next()
            .map_err(|error| fail("bad PE import descriptor", error))?
        {
            let name = table
                .name(descriptor.name.get(LittleEndian))
                .map_err(|error| fail("bad PE import name", error))?;
            needed.push(utf8(name));
        }
    }
    Ok(Image {
        format: Format::Pe,
        machine: pe_machine_name(headers.file_header().machine.get(LittleEndian)),
        abi: Abi::Msvc {
            subsystem: pe_subsystem(headers.optional_header().subsystem()),
        },
        needed: sorted(needed),
    })
}

#[cfg(test)]
mod tests;
