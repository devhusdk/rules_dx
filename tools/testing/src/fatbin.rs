//! Reads CUDA device code and dynamic dependencies from ELF and COFF/PE objects, archives and binaries.

pub const FATBIN_MAGIC: u32 = 0xBA55_ED50;
const FATBIN_VERSION: u16 = 1;
const FATBIN_HEADER_SIZE: usize = 16;
const ENTRY_VERSION: u16 = 0x0101;
const ENTRY_MIN_HEADER: usize = 32;
const FATBIN_SECTIONS: [&str; 3] = [".nv_fatbin", ".nv_fatb", "__nv_relfatbin"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Ptx,
    Elf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Entry {
    pub kind: Kind,
    pub arch: u32,
}

fn u16_at(bytes: &[u8], at: usize) -> Result<u16, String> {
    bytes
        .get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or_else(|| format!("truncated u16 at offset {at}"))
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32, String> {
    bytes
        .get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| format!("truncated u32 at offset {at}"))
}

fn u64_at(bytes: &[u8], at: usize) -> Result<u64, String> {
    let b = bytes
        .get(at..at + 8)
        .ok_or_else(|| format!("truncated u64 at offset {at}"))?;
    let mut word = [0u8; 8];
    word.copy_from_slice(b);
    Ok(u64::from_le_bytes(word))
}

fn to_usize(value: u64) -> Result<usize, String> {
    usize::try_from(value).map_err(|_| format!("value {value} does not fit in usize"))
}

/// Decodes every fatbin in a section payload into its entries.
pub fn fatbin_entries(data: &[u8]) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    let mut at = 0;
    while at + FATBIN_HEADER_SIZE <= data.len() {
        let magic = u32_at(data, at)?;
        if magic == 0 {
            at += 8;
            continue;
        }
        if magic != FATBIN_MAGIC {
            return Err(format!("bad fatbin magic {magic:#x} at offset {at}"));
        }
        let version = u16_at(data, at + 4)?;
        if version != FATBIN_VERSION {
            return Err(format!(
                "unsupported fatbin version {version} at offset {at}"
            ));
        }
        let header_size = usize::from(u16_at(data, at + 6)?);
        if header_size != FATBIN_HEADER_SIZE {
            return Err(format!(
                "unsupported fatbin header size {header_size} at offset {at}"
            ));
        }
        let body_size = to_usize(u64_at(data, at + 8)?)?;
        let body_start = at + header_size;
        let body_end = body_start
            .checked_add(body_size)
            .filter(|end| *end <= data.len())
            .ok_or_else(|| format!("fatbin at offset {at} overruns its section"))?;
        let mut entry_at = body_start;
        while entry_at < body_end {
            let kind = match u16_at(data, entry_at)? {
                1 => Kind::Ptx,
                2 => Kind::Elf,
                other => {
                    return Err(format!(
                        "unknown fatbin entry kind {other} at offset {entry_at}"
                    ))
                }
            };
            let entry_version = u16_at(data, entry_at + 2)?;
            if entry_version != ENTRY_VERSION {
                return Err(format!(
                    "unsupported fatbin entry version {entry_version:#x} at offset {entry_at}"
                ));
            }
            let entry_header = to_usize(u64::from(u32_at(data, entry_at + 4)?))?;
            if entry_header < ENTRY_MIN_HEADER {
                return Err(format!(
                    "fatbin entry header {entry_header} too small at offset {entry_at}"
                ));
            }
            let payload = to_usize(u64_at(data, entry_at + 8)?)?;
            let arch = u32_at(data, entry_at + 28)?;
            entries.push(Entry { kind, arch });
            entry_at = entry_at
                .checked_add(entry_header)
                .and_then(|v| v.checked_add(payload))
                .filter(|end| *end <= body_end)
                .ok_or_else(|| format!("fatbin entry at offset {entry_at} overruns its fatbin"))?;
        }
        at = body_end;
    }
    Ok(entries)
}

struct Section<'a> {
    name: String,
    kind: u32,
    link: u32,
    data: &'a [u8],
}

fn elf_sections(bytes: &[u8]) -> Result<Vec<Section<'_>>, String> {
    if bytes.get(0..4) != Some(b"\x7fELF".as_slice()) {
        return Err("not an ELF file".to_string());
    }
    if bytes.get(4) != Some(&2) || bytes.get(5) != Some(&1) {
        return Err("only little-endian ELF64 is supported".to_string());
    }
    let shoff = to_usize(u64_at(bytes, 0x28)?)?;
    let shentsize = usize::from(u16_at(bytes, 0x3A)?);
    let shnum = usize::from(u16_at(bytes, 0x3C)?);
    let shstrndx = usize::from(u16_at(bytes, 0x3E)?);
    if shoff == 0 || shnum == 0 {
        return Ok(Vec::new());
    }
    if shentsize < 64 {
        return Err(format!("ELF section header size {shentsize} is too small"));
    }
    let mut raw = Vec::with_capacity(shnum);
    for index in 0..shnum {
        let at = shoff + index * shentsize;
        let name = u32_at(bytes, at)?;
        let kind = u32_at(bytes, at + 4)?;
        let offset = to_usize(u64_at(bytes, at + 0x18)?)?;
        let size = to_usize(u64_at(bytes, at + 0x20)?)?;
        let link = u32_at(bytes, at + 0x28)?;
        let data: &[u8] = if kind == 8 {
            &[]
        } else {
            bytes
                .get(
                    offset
                        ..offset
                            .checked_add(size)
                            .ok_or("ELF section size overflows")?,
                )
                .ok_or_else(|| format!("ELF section {index} overruns the file"))?
        };
        raw.push((name, kind, link, data));
    }
    let names = raw
        .get(shstrndx)
        .map(|s| s.3)
        .ok_or("ELF section name table is missing")?;
    raw.into_iter()
        .map(|(name, kind, link, data)| {
            Ok(Section {
                name: c_string(names, to_usize(u64::from(name))?)?,
                kind,
                link,
                data,
            })
        })
        .collect()
}

fn c_string(table: &[u8], at: usize) -> Result<String, String> {
    let tail = table
        .get(at..)
        .ok_or_else(|| format!("string offset {at} is out of range"))?;
    let end = tail.iter().position(|b| *b == 0).unwrap_or(tail.len());
    String::from_utf8(tail[..end].to_vec()).map_err(|e| e.to_string())
}

fn elf_device_code(bytes: &[u8]) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    for section in elf_sections(bytes)? {
        if FATBIN_SECTIONS.contains(&section.name.as_str()) {
            entries.extend(fatbin_entries(section.data)?);
        }
    }
    Ok(entries)
}

/// Returns the members of a System V or GNU ar archive.
pub fn archive_members(bytes: &[u8]) -> Result<Vec<(String, &[u8])>, String> {
    let mut members = Vec::new();
    let mut long_names: &[u8] = &[];
    let mut at = 8;
    while at + 60 <= bytes.len() {
        let header = &bytes[at..at + 60];
        if &header[58..60] != b"`\n" {
            return Err(format!("bad ar member header at offset {at}"));
        }
        let raw_name = String::from_utf8_lossy(&header[0..16])
            .trim_end()
            .to_string();
        let size: usize = String::from_utf8_lossy(&header[48..58])
            .trim()
            .parse()
            .map_err(|e| format!("bad ar member size at offset {at}: {e}"))?;
        let start = at + 60;
        let data = bytes
            .get(start..start + size)
            .ok_or_else(|| format!("ar member at offset {at} overruns the archive"))?;
        if raw_name == "//" {
            long_names = data;
        } else if raw_name != "/" && raw_name != "/SYM64/" {
            let name = match raw_name.strip_prefix('/') {
                Some(offset) => {
                    let offset: usize = offset
                        .parse()
                        .map_err(|e| format!("bad ar long name {raw_name}: {e}"))?;
                    let tail = long_names
                        .get(offset..)
                        .ok_or_else(|| format!("ar long name {raw_name} out of range"))?;
                    let end = tail.iter().position(|b| *b == b'\n').unwrap_or(tail.len());
                    String::from_utf8_lossy(&tail[..end])
                        .trim_end_matches('/')
                        .to_string()
                }
                None => raw_name.trim_end_matches('/').to_string(),
            };
            members.push((name, data));
        }
        at = start + size + (size % 2);
    }
    Ok(members)
}

const COFF_MACHINES: [u16; 3] = [0x8664, 0xAA64, 0x014C];

fn is_bigobj(bytes: &[u8]) -> bool {
    bytes.len() >= 56 && bytes[..4] == [0, 0, 0xFF, 0xFF] && bytes[4..6] != [0, 0]
}

fn is_coff_object(bytes: &[u8]) -> bool {
    is_bigobj(bytes)
        || (bytes.len() >= 20
            && COFF_MACHINES.contains(&u16::from_le_bytes([bytes[0], bytes[1]]))
            && bytes[2..4] != [0, 0])
}

fn is_pe_image(bytes: &[u8]) -> bool {
    bytes.starts_with(b"MZ")
}

struct PeLayout {
    optional: usize,
    sections: usize,
    count: usize,
    symbols: usize,
    symbol_count: usize,
    symbol_size: usize,
}

fn pe_layout(bytes: &[u8]) -> Result<PeLayout, String> {
    if is_bigobj(bytes) {
        return Ok(PeLayout {
            optional: 56,
            sections: 56,
            count: to_usize(u64::from(u32_at(bytes, 44)?))?,
            symbols: to_usize(u64::from(u32_at(bytes, 48)?))?,
            symbol_count: to_usize(u64::from(u32_at(bytes, 52)?))?,
            symbol_size: 20,
        });
    }
    let (coff, optional_size) = if is_pe_image(bytes) {
        let pe = to_usize(u64::from(u32_at(bytes, 0x3C)?))?;
        if bytes.get(pe..pe + 4) != Some(b"PE\0\0".as_slice()) {
            return Err("not a PE file".to_string());
        }
        let coff = pe + 4;
        (coff, usize::from(u16_at(bytes, coff + 16)?))
    } else {
        (0, usize::from(u16_at(bytes, 16)?))
    };
    Ok(PeLayout {
        optional: coff + 20,
        sections: coff + 20 + optional_size,
        count: usize::from(u16_at(bytes, coff + 2)?),
        symbols: to_usize(u64::from(u32_at(bytes, coff + 8)?))?,
        symbol_count: to_usize(u64::from(u32_at(bytes, coff + 12)?))?,
        symbol_size: 18,
    })
}

struct CoffSection<'a> {
    section: Section<'a>,
    virtual_address: usize,
    virtual_size: usize,
    raw_offset: usize,
    raw_size: usize,
}

fn coff_sections(bytes: &[u8]) -> Result<Vec<CoffSection<'_>>, String> {
    let layout = pe_layout(bytes)?;
    let strings = layout
        .symbols
        .checked_add(layout.symbol_count * layout.symbol_size)
        .filter(|_| layout.symbols != 0)
        .and_then(|at| bytes.get(at..))
        .unwrap_or(&[]);
    let mut out = Vec::with_capacity(layout.count);
    for index in 0..layout.count {
        let at = layout.sections + index * 40;
        let raw_name = bytes
            .get(at..at + 8)
            .ok_or_else(|| format!("truncated COFF section header {index}"))?;
        let end = raw_name.iter().position(|b| *b == 0).unwrap_or(8);
        let short = String::from_utf8_lossy(&raw_name[..end]).to_string();
        let name = match short.strip_prefix('/').map(str::parse::<usize>) {
            Some(Ok(offset)) => c_string(strings, offset)?,
            _ => short,
        };
        let virtual_size = to_usize(u64::from(u32_at(bytes, at + 8)?))?;
        let virtual_address = to_usize(u64::from(u32_at(bytes, at + 12)?))?;
        let raw_size = to_usize(u64::from(u32_at(bytes, at + 16)?))?;
        let raw_offset = to_usize(u64::from(u32_at(bytes, at + 20)?))?;
        let characteristics = u32_at(bytes, at + 36)?;
        let data: &[u8] = if raw_size == 0 || characteristics & 0x80 != 0 {
            &[]
        } else {
            bytes
                .get(raw_offset..raw_offset.saturating_add(raw_size))
                .ok_or_else(|| format!("COFF section {name} overruns the file"))?
        };
        out.push(CoffSection {
            section: Section {
                name,
                kind: 1,
                link: 0,
                data,
            },
            virtual_address,
            virtual_size,
            raw_offset,
            raw_size,
        });
    }
    Ok(out)
}

fn coff_device_code(bytes: &[u8]) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    for section in coff_sections(bytes)? {
        if FATBIN_SECTIONS.contains(&section.section.name.as_str()) {
            entries.extend(fatbin_entries(section.section.data)?);
        }
    }
    Ok(entries)
}

fn object_device_code(bytes: &[u8]) -> Result<Vec<Entry>, String> {
    if bytes.starts_with(b"\x7fELF") {
        elf_device_code(bytes)
    } else if is_pe_image(bytes) || is_coff_object(bytes) {
        coff_device_code(bytes)
    } else {
        Err("not an ELF, COFF or PE file".to_string())
    }
}

/// Returns the device code embedded in an ELF, COFF or PE file, or in every object member of an
/// ar archive.
pub fn device_code(bytes: &[u8]) -> Result<Vec<Entry>, String> {
    if bytes.starts_with(b"!<arch>\n") {
        let mut entries = Vec::new();
        for (name, data) in archive_members(bytes)? {
            if data.starts_with(b"\x7fELF") || is_coff_object(data) {
                entries.extend(object_device_code(data).map_err(|e| format!("{name}: {e}"))?);
            }
        }
        Ok(entries)
    } else {
        object_device_code(bytes)
    }
}

fn pe_imports(bytes: &[u8]) -> Result<Vec<String>, String> {
    let layout = pe_layout(bytes)?;
    let directories = match u16_at(bytes, layout.optional)? {
        0x20B => layout.optional + 112,
        0x10B => layout.optional + 96,
        other => return Err(format!("unknown PE optional header magic {other:#x}")),
    };
    let table = to_usize(u64::from(u32_at(bytes, directories + 8)?))?;
    if table == 0 {
        return Ok(Vec::new());
    }
    let sections = coff_sections(bytes)?;
    let offset = |rva: usize| -> Result<usize, String> {
        sections
            .iter()
            .find(|s| {
                rva >= s.virtual_address && rva < s.virtual_address + s.virtual_size.max(s.raw_size)
            })
            .map(|s| rva - s.virtual_address + s.raw_offset)
            .ok_or_else(|| format!("PE address {rva:#x} is not in any section"))
    };
    let mut at = offset(table)?;
    let mut needed = Vec::new();
    loop {
        let descriptor = bytes
            .get(at..at + 20)
            .ok_or("PE import table overruns the file")?;
        if descriptor.iter().all(|b| *b == 0) {
            break;
        }
        let name = to_usize(u64::from(u32_at(descriptor, 12)?))?;
        needed.push(c_string(bytes, offset(name)?)?);
        at += 20;
    }
    Ok(needed)
}

/// Returns the DT_NEEDED entries of an ELF file or the imported DLL names of a PE file.
pub fn needed_libraries(bytes: &[u8]) -> Result<Vec<String>, String> {
    if is_pe_image(bytes) {
        return pe_imports(bytes);
    }
    let sections = elf_sections(bytes)?;
    let mut needed = Vec::new();
    for section in sections.iter().filter(|s| s.kind == 6) {
        let strings = sections
            .get(to_usize(u64::from(section.link))?)
            .map(|s| s.data)
            .ok_or("dynamic section has no string table")?;
        let (entries, _) = section.data.as_chunks::<16>();
        for chunk in entries {
            let chunk = chunk.as_slice();
            let tag = u64_at(chunk, 0)?;
            if tag == 0 {
                break;
            }
            if tag == 1 {
                needed.push(c_string(strings, to_usize(u64_at(chunk, 8)?)?)?);
            }
        }
    }
    Ok(needed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: u16, arch: u32, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(kind.to_le_bytes());
        out.extend(ENTRY_VERSION.to_le_bytes());
        out.extend(64u32.to_le_bytes());
        out.extend((payload.len() as u64).to_le_bytes());
        out.extend([0u8; 12]);
        out.extend(arch.to_le_bytes());
        out.resize(64, 0);
        out.extend(payload);
        out
    }

    fn fatbin(entries: &[Vec<u8>]) -> Vec<u8> {
        let body: Vec<u8> = entries.concat();
        let mut out = Vec::new();
        out.extend(FATBIN_MAGIC.to_le_bytes());
        out.extend(FATBIN_VERSION.to_le_bytes());
        out.extend((FATBIN_HEADER_SIZE as u16).to_le_bytes());
        out.extend((body.len() as u64).to_le_bytes());
        out.extend(body);
        out
    }

    fn elf(sections: &[(&str, u32, u32, Vec<u8>)]) -> Vec<u8> {
        let mut names = vec![0u8];
        let mut name_offsets = Vec::new();
        for (name, _, _, _) in sections {
            name_offsets.push(names.len() as u32);
            names.extend(name.as_bytes());
            names.push(0);
        }
        let shstrtab_name = names.len() as u32;
        names.extend(b".shstrtab\0");
        let mut out = vec![0u8; 64];
        out[0..4].copy_from_slice(b"\x7fELF");
        out[4] = 2;
        out[5] = 1;
        let mut placed = Vec::new();
        for (_, _, _, data) in sections {
            placed.push((out.len() as u64, data.len() as u64));
            out.extend(data);
        }
        let names_at = out.len() as u64;
        out.extend(&names);
        while out.len() % 8 != 0 {
            out.push(0);
        }
        let shoff = out.len() as u64;
        out.extend([0u8; 64]);
        for (index, (_, kind, link, _)) in sections.iter().enumerate() {
            let mut header = vec![0u8; 64];
            header[0..4].copy_from_slice(&name_offsets[index].to_le_bytes());
            header[4..8].copy_from_slice(&kind.to_le_bytes());
            header[0x18..0x20].copy_from_slice(&placed[index].0.to_le_bytes());
            header[0x20..0x28].copy_from_slice(&placed[index].1.to_le_bytes());
            header[0x28..0x2C].copy_from_slice(&link.to_le_bytes());
            out.extend(header);
        }
        let mut header = vec![0u8; 64];
        header[0..4].copy_from_slice(&shstrtab_name.to_le_bytes());
        header[4..8].copy_from_slice(&3u32.to_le_bytes());
        header[0x18..0x20].copy_from_slice(&names_at.to_le_bytes());
        header[0x20..0x28].copy_from_slice(&(names.len() as u64).to_le_bytes());
        out.extend(header);
        let shnum = sections.len() as u16 + 2;
        out[0x28..0x30].copy_from_slice(&shoff.to_le_bytes());
        out[0x3A..0x3C].copy_from_slice(&64u16.to_le_bytes());
        out[0x3C..0x3E].copy_from_slice(&shnum.to_le_bytes());
        out[0x3E..0x40].copy_from_slice(&(shnum - 1).to_le_bytes());
        out
    }

    fn ar(members: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut out = b"!<arch>\n".to_vec();
        for (name, data) in members {
            out.extend(
                format!(
                    "{:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n",
                    format!("{name}/"),
                    0,
                    0,
                    0,
                    644,
                    data.len()
                )
                .as_bytes(),
            );
            out.extend(data);
            if data.len() % 2 == 1 {
                out.push(b'\n');
            }
        }
        out
    }

    fn fixture_fatbin() -> Vec<u8> {
        fatbin(&[
            entry(2, 89, b"sass-89!"),
            entry(1, 89, b"ptx"),
            entry(2, 120, b"sass-120"),
        ])
    }

    #[test]
    fn fatbin_decodes_kinds_and_archs() {
        let entries = fatbin_entries(&fixture_fatbin()).unwrap();
        assert_eq!(
            entries,
            vec![
                Entry {
                    kind: Kind::Elf,
                    arch: 89
                },
                Entry {
                    kind: Kind::Ptx,
                    arch: 89
                },
                Entry {
                    kind: Kind::Elf,
                    arch: 120
                },
            ]
        );
    }

    #[test]
    fn fatbin_skips_alignment_padding_between_fatbins() {
        let mut data = fixture_fatbin();
        data.extend([0u8; 8]);
        data.extend(fatbin(&[entry(2, 75, b"")]));
        assert_eq!(fatbin_entries(&data).unwrap().len(), 4);
    }

    #[test]
    fn fatbin_rejects_bad_magic() {
        let mut data = fixture_fatbin();
        data[0] = 0x51;
        assert!(fatbin_entries(&data)
            .unwrap_err()
            .contains("bad fatbin magic"));
    }

    #[test]
    fn fatbin_rejects_unknown_header_version() {
        let mut data = fixture_fatbin();
        data[4] = 2;
        assert!(fatbin_entries(&data)
            .unwrap_err()
            .contains("unsupported fatbin version"));
    }

    #[test]
    fn fatbin_rejects_truncated_body() {
        let mut data = fixture_fatbin();
        data.truncate(data.len() - 4);
        assert!(fatbin_entries(&data).unwrap_err().contains("overruns"));
    }

    #[test]
    fn fatbin_device_code_reads_elf_sections() {
        let object = elf(&[
            (".text", 1, 0, vec![0x90; 4]),
            (".nv_fatbin", 1, 0, fixture_fatbin()),
        ]);
        assert_eq!(device_code(&object).unwrap().len(), 3);
        let rdc = elf(&[("__nv_relfatbin", 1, 0, fatbin(&[entry(2, 86, b"")]))]);
        assert_eq!(
            device_code(&rdc).unwrap(),
            vec![Entry {
                kind: Kind::Elf,
                arch: 86
            }]
        );
    }

    #[test]
    fn fatbin_device_code_reads_archive_members() {
        let first = elf(&[(".nv_fatbin", 1, 0, fixture_fatbin())]);
        let second = elf(&[(".text", 1, 0, vec![0xC3])]);
        let archive = ar(&[("a.o", first), ("b.o", second)]);
        let names: Vec<String> = archive_members(&archive)
            .unwrap()
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert_eq!(names, vec!["a.o", "b.o"]);
        assert_eq!(device_code(&archive).unwrap().len(), 3);
    }

    #[test]
    fn fatbin_needed_libraries_reads_dynamic_section() {
        let strings = b"\0libc.so.6\0libcudart.so.13\0".to_vec();
        let mut dynamic = Vec::new();
        for (tag, value) in [(1u64, 1u64), (1, 11), (0, 0)] {
            dynamic.extend(tag.to_le_bytes());
            dynamic.extend(value.to_le_bytes());
        }
        let binary = elf(&[(".dynstr", 3, 0, strings), (".dynamic", 6, 1, dynamic)]);
        assert_eq!(
            needed_libraries(&binary).unwrap(),
            vec!["libc.so.6", "libcudart.so.13"]
        );
    }

    fn coff(machine: u16, sections: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut strings = vec![0u8; 4];
        let mut out = vec![0u8; 20];
        out[0..2].copy_from_slice(&machine.to_le_bytes());
        out[2..4].copy_from_slice(&(sections.len() as u16).to_le_bytes());
        let mut at = 20 + 40 * sections.len();
        let mut headers = Vec::new();
        let mut bodies: Vec<u8> = Vec::new();
        for (name, data) in sections {
            let mut header = vec![0u8; 40];
            if name.len() > 8 {
                let field = format!("/{}", strings.len());
                header[..field.len()].copy_from_slice(field.as_bytes());
                strings.extend(name.as_bytes());
                strings.push(0);
            } else {
                header[..name.len()].copy_from_slice(name.as_bytes());
            }
            header[16..20].copy_from_slice(&(data.len() as u32).to_le_bytes());
            header[20..24].copy_from_slice(&(at as u32).to_le_bytes());
            headers.extend(header);
            bodies.extend(data);
            at += data.len();
        }
        out[8..12].copy_from_slice(&(at as u32).to_le_bytes());
        out.extend(headers);
        out.extend(bodies);
        let len = strings.len() as u32;
        strings[0..4].copy_from_slice(&len.to_le_bytes());
        out.extend(strings);
        out
    }

    #[test]
    fn fatbin_device_code_reads_coff_sections() {
        let object = coff(
            0x8664,
            &[(".text", vec![0xC3]), (".nv_fatbin", fixture_fatbin())],
        );
        assert_eq!(device_code(&object).unwrap().len(), 3);
        let rdc = coff(0xAA64, &[("__nv_relfatbin", fatbin(&[entry(2, 86, b"")]))]);
        assert_eq!(device_code(&rdc).unwrap().len(), 1);
        let short = coff(0x8664, &[(".nv_fatb", fixture_fatbin())]);
        assert_eq!(device_code(&short).unwrap().len(), 3);
    }

    fn bigobj(sections: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let regular = coff(0x8664, sections);
        let count = sections.len();
        let mut out = vec![0u8; 56];
        out[2..4].copy_from_slice(&[0xFF, 0xFF]);
        out[4..6].copy_from_slice(&2u16.to_le_bytes());
        out[6..8].copy_from_slice(&0x8664u16.to_le_bytes());
        out[44..48].copy_from_slice(&(count as u32).to_le_bytes());
        let delta = 56 - 20;
        let symbols = u32::from_le_bytes(regular[8..12].try_into().unwrap()) as usize + delta;
        out[48..52].copy_from_slice(&(symbols as u32).to_le_bytes());
        let mut rest = regular[20..].to_vec();
        for index in 0..count {
            let at = index * 40 + 20;
            let offset = u32::from_le_bytes(rest[at..at + 4].try_into().unwrap()) + delta as u32;
            rest[at..at + 4].copy_from_slice(&offset.to_le_bytes());
        }
        out.extend(rest);
        out
    }

    #[test]
    fn fatbin_device_code_reads_bigobj_members() {
        let object = bigobj(&[
            (".text", vec![0xC3]),
            ("__nv_relfatbin", fixture_fatbin()),
        ]);
        assert!(is_coff_object(&object));
        assert_eq!(device_code(&object).unwrap().len(), 3);
        assert_eq!(device_code(&ar(&[("a.obj", object)])).unwrap().len(), 3);
    }

    #[test]
    fn fatbin_device_code_reads_msvc_lib_members() {
        let object = coff(0x8664, &[(".nv_fatbin", fixture_fatbin())]);
        let import_member = vec![
            0, 0, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        let archive = ar(&[
            ("", vec![0, 0, 0, 0]),
            ("", vec![0, 0, 0, 0]),
            ("a.obj", object),
            ("d.dll", import_member),
        ]);
        assert_eq!(device_code(&archive).unwrap().len(), 3);
    }

    fn pe(imports: &[&str]) -> Vec<u8> {
        let mut out = vec![0u8; 0x40];
        out[0..2].copy_from_slice(b"MZ");
        out[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        out.extend(b"PE\0\0");
        let mut coff = vec![0u8; 20];
        coff[0..2].copy_from_slice(&0x8664u16.to_le_bytes());
        coff[2..4].copy_from_slice(&1u16.to_le_bytes());
        coff[16..18].copy_from_slice(&240u16.to_le_bytes());
        out.extend(coff);
        let mut optional = vec![0u8; 240];
        optional[0..2].copy_from_slice(&0x20Bu16.to_le_bytes());
        optional[112 + 8..112 + 12].copy_from_slice(&0x1000u32.to_le_bytes());
        out.extend(optional);
        let raw = 0x200usize;
        let mut section = vec![0u8; 40];
        section[..6].copy_from_slice(b".idata");
        section[8..12].copy_from_slice(&0x400u32.to_le_bytes());
        section[12..16].copy_from_slice(&0x1000u32.to_le_bytes());
        section[16..20].copy_from_slice(&0x400u32.to_le_bytes());
        section[20..24].copy_from_slice(&(raw as u32).to_le_bytes());
        out.extend(section);
        out.resize(raw, 0);
        let mut body = vec![0u8; 20 * (imports.len() + 1)];
        for (index, name) in imports.iter().enumerate() {
            let rva = 0x1000 + body.len() as u32;
            body[index * 20 + 12..index * 20 + 16].copy_from_slice(&rva.to_le_bytes());
            body.extend(name.as_bytes());
            body.push(0);
        }
        body.resize(0x400, 0);
        out.extend(body);
        out
    }

    #[test]
    fn fatbin_needed_libraries_reads_pe_imports() {
        let binary = pe(&["KERNEL32.dll", "cudart64_13.dll"]);
        assert_eq!(
            needed_libraries(&binary).unwrap(),
            vec!["KERNEL32.dll", "cudart64_13.dll"]
        );
        assert!(device_code(&binary).unwrap().is_empty());
    }

    #[test]
    fn fatbin_rejects_non_elf() {
        assert!(device_code(b"#!/bin/sh\n")
            .unwrap_err()
            .contains("not an ELF"));
    }
}
