//! Checks that a stripped ELF binary and its debug file belong together.

use object::{Architecture, Object};

/// The build-id and architecture shared by a binary and its debug file.
#[derive(Debug, PartialEq, Eq)]
pub struct Pair {
    pub build_id: String,
    pub arch: String,
}

struct Elf {
    build_id: Vec<u8>,
    arch: Architecture,
    machine: u16,
}

fn read(bytes: &[u8], what: &str) -> Result<Elf, String> {
    let file = object::File::parse(bytes).map_err(|e| {
        if bytes.starts_with(b"\x7fELF") {
            format!("{what} is corrupt: {e}")
        } else {
            format!("{what} is not an ELF file")
        }
    })?;
    if file.format() != object::BinaryFormat::Elf {
        return Err(format!("{what} is not an ELF file"));
    }
    let build_id = file
        .build_id()
        .map_err(|e| format!("{what} is corrupt: {e}"))?
        .ok_or_else(|| format!("{what} has no build-id"))?
        .to_vec();
    let raw = [bytes[18], bytes[19]];
    let machine = if file.is_little_endian() {
        u16::from_le_bytes(raw)
    } else {
        u16::from_be_bytes(raw)
    };
    Ok(Elf {
        build_id,
        arch: file.architecture(),
        machine,
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Returns the shared build-id and arch, or says why the two files do not match.
pub fn check_pair(binary: &[u8], debug: &[u8]) -> Result<Pair, String> {
    let bin = read(binary, "binary")?;
    let dbg = read(debug, "debug file")?;
    if bin.build_id != dbg.build_id {
        return Err(format!(
            "build-id mismatch: binary {} debug file {}",
            hex(&bin.build_id),
            hex(&dbg.build_id)
        ));
    }
    if bin.machine != dbg.machine || bin.arch != dbg.arch {
        return Err(format!(
            "arch mismatch: binary {:?} debug file {:?}",
            bin.arch, dbg.arch
        ));
    }
    Ok(Pair {
        build_id: hex(&bin.build_id),
        arch: format!("{:?}", bin.arch).to_lowercase(),
    })
}

#[cfg(test)]
mod tests;
