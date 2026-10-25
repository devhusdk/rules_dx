//! Reads CUDA device code and dynamic dependencies from ELF objects, archives and binaries.

pub const FATBIN_MAGIC: u32 = 0xBA55_ED50;
const FATBIN_VERSION: u16 = 1;
const FATBIN_HEADER_SIZE: usize = 16;
const ENTRY_VERSION: u16 = 0x0101;
const ENTRY_MIN_HEADER: usize = 32;
const FATBIN_SECTIONS: [&str; 2] = [".nv_fatbin", "__nv_relfatbin"];

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

/// Returns the device code embedded in an ELF file or in every ELF member of an ar archive.
pub fn device_code(bytes: &[u8]) -> Result<Vec<Entry>, String> {
    if bytes.starts_with(b"!<arch>\n") {
        let mut entries = Vec::new();
        for (name, data) in archive_members(bytes)? {
            if data.starts_with(b"\x7fELF") {
                entries.extend(elf_device_code(data).map_err(|e| format!("{name}: {e}"))?);
            }
        }
        Ok(entries)
    } else {
        elf_device_code(bytes)
    }
}

/// Returns the DT_NEEDED entries of an ELF file.
pub fn needed_libraries(bytes: &[u8]) -> Result<Vec<String>, String> {
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

    #[test]
    fn fatbin_rejects_non_elf() {
        assert!(device_code(b"#!/bin/sh\n")
            .unwrap_err()
            .contains("not an ELF"));
    }
}
