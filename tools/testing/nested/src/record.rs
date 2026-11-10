//! Record mode: reads the downloads a nested Bazel logged with `--experimental_workspace_rules_log_file`.

use std::collections::BTreeSet;

use crate::cache;

/// The `WorkspaceEvent` field of a `DownloadEvent`.
const DOWNLOAD: u64 = 4;
/// The `WorkspaceEvent` field of a `DownloadAndExtractEvent`.
const DOWNLOAD_AND_EXTRACT: u64 = 5;

struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn varint(&mut self) -> Option<u64> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let (&byte, rest) = self.bytes.split_first()?;
            self.bytes = rest;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Some(value);
            }
        }
        None
    }

    fn take(&mut self, len: u64) -> Option<&'a [u8]> {
        let len = usize::try_from(len).ok()?;
        if len > self.bytes.len() {
            return None;
        }
        let (head, rest) = self.bytes.split_at(len);
        self.bytes = rest;
        Some(head)
    }

    /// The next field as (number, length-delimited payload); other wire types give an empty payload.
    fn field(&mut self) -> Option<(u64, &'a [u8])> {
        let tag = self.varint()?;
        let payload: &[u8] = match tag & 7 {
            0 => {
                self.varint()?;
                &[]
            }
            1 => self.take(8)?,
            2 => {
                let len = self.varint()?;
                self.take(len)?
            }
            5 => self.take(4)?,
            _ => return None,
        };
        let payload = if tag & 7 == 2 { payload } else { &[] };
        Some((tag >> 3, payload))
    }
}

/// The first URL and the checksum of one download message; `integrity` is the field number of the integrity.
fn download(message: &[u8], integrity: u64) -> Option<(String, String)> {
    let mut reader = Reader { bytes: message };
    let mut url = None;
    let mut sha256 = String::new();
    let mut sri = String::new();
    while !reader.bytes.is_empty() {
        let (number, payload) = reader.field()?;
        let text = || String::from_utf8_lossy(payload).into_owned();
        match number {
            1 if url.is_none() => url = Some(text()),
            3 => sha256 = text(),
            n if n == integrity => sri = text(),
            _ => {}
        }
    }
    let sha256 = if sha256.is_empty() {
        cache::integrity_hex(&sri)?
    } else {
        sha256
    };
    Some((url?, sha256))
}

/// The `(url, sha256)` of every download with a checksum in a delimited `WorkspaceEvent` log.
pub fn downloads(log: &[u8]) -> Vec<(String, String)> {
    let mut found = BTreeSet::new();
    let mut reader = Reader { bytes: log };
    while !reader.bytes.is_empty() {
        let Some(len) = reader.varint() else { break };
        let Some(event) = reader.take(len) else { break };
        let mut fields = Reader { bytes: event };
        while !fields.bytes.is_empty() {
            let Some((number, payload)) = fields.field() else {
                break;
            };
            let entry = match number {
                DOWNLOAD => download(payload, 5),
                DOWNLOAD_AND_EXTRACT => download(payload, 6),
                _ => None,
            };
            found.extend(entry);
        }
    }
    found.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(number: u8, payload: &[u8]) -> Vec<u8> {
        let mut out = vec![(number << 3) | 2, u8::try_from(payload.len()).expect("short")];
        out.extend_from_slice(payload);
        out
    }

    fn event(number: u8, message: &[u8]) -> Vec<u8> {
        let mut inner = field(1, b"location");
        inner.extend(field(number, message));
        let mut out = vec![u8::try_from(inner.len()).expect("short")];
        out.extend(inner);
        out
    }

    const SHA: &str = "4e50948fbb8564a8b3a3b1f354cd9ec7ffdfe304f63c6a5feb1fdbcfbfc782bd";

    #[test]
    fn reads_download_and_download_and_extract() {
        let mut first = field(1, b"https://a/x");
        first.extend(field(1, b"https://mirror/x"));
        first.extend(field(3, SHA.as_bytes()));
        let mut second = field(1, b"https://b/y");
        second.extend(field(3, SHA.as_bytes()));
        let mut log = event(4, &first);
        log.extend(event(5, &second));
        assert_eq!(
            downloads(&log),
            vec![
                ("https://a/x".to_owned(), SHA.to_owned()),
                ("https://b/y".to_owned(), SHA.to_owned()),
            ]
        );
    }

    #[test]
    fn falls_back_to_the_integrity_and_skips_unchecked_downloads() {
        let mut with_integrity = field(1, b"https://a/x");
        with_integrity.extend(field(5, b"sha256-Tk6Uj7uFZLijo7PzGzz+z9y+0mP2eoEw+32y+KHwEGM="));
        let unchecked = field(1, b"https://c/z");
        let mut log = event(4, &with_integrity);
        log.extend(event(4, &unchecked));
        let found = downloads(&log);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "https://a/x");
        assert_eq!(found[0].1.len(), 64);
    }

    #[test]
    fn a_truncated_log_keeps_what_was_read() {
        let mut message = field(1, b"https://a/x");
        message.extend(field(3, SHA.as_bytes()));
        let mut log = event(4, &message);
        log.extend([9, 1]);
        assert_eq!(downloads(&log).len(), 1);
    }
}
