use std::collections::{BTreeMap, BTreeSet};

use crate::cache;

const CONTEXT: u64 = 2;
const DOWNLOAD: u64 = 4;
const DOWNLOAD_AND_EXTRACT: u64 = 5;
const REPOSITORY: &str = "repository @@";

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

struct Download {
    url: Option<String>,
    digest: Option<String>,
}

fn parse_download(message: &[u8], integrity: u64) -> Option<Download> {
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
    let digest = if sha256.is_empty() {
        cache::integrity_hex(&sri)
    } else {
        Some(sha256)
    };
    Some(Download { url, digest })
}

fn download(message: &[u8], integrity: u64) -> Option<(String, String)> {
    let Download { url, digest } = parse_download(message, integrity)?;
    Some((url?, digest?))
}

#[must_use]
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

#[must_use]
pub fn contexts(log: &[u8]) -> Vec<String> {
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
            if number == CONTEXT {
                found.insert(String::from_utf8_lossy(payload).into_owned());
            }
        }
    }
    found.into_iter().collect()
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Fetches {
    pub ran: BTreeSet<String>,
    pub downloaded: BTreeSet<String>,
    pub archives: BTreeMap<String, String>,
    pub unchecked: BTreeSet<String>,
}

impl Fetches {
    pub fn add(&mut self, log: &[u8]) {
        let mut reader = Reader { bytes: log };
        while !reader.bytes.is_empty() {
            let Some(len) = reader.varint() else { break };
            let Some(event) = reader.take(len) else { break };
            let mut fields = Reader { bytes: event };
            let mut context = String::new();
            let mut downloads = Vec::new();
            while !fields.bytes.is_empty() {
                let Some((number, payload)) = fields.field() else {
                    break;
                };
                match number {
                    CONTEXT => context = String::from_utf8_lossy(payload).into_owned(),
                    DOWNLOAD => downloads.push(parse_download(payload, 5)),
                    DOWNLOAD_AND_EXTRACT => downloads.push(parse_download(payload, 6)),
                    _ => {}
                }
            }
            let repo = context.strip_prefix(REPOSITORY).map(str::to_owned);
            let owner = repo.clone().unwrap_or(context);
            for download in downloads.iter().flatten() {
                match (&download.digest, &download.url) {
                    (Some(digest), _) => {
                        self.archives
                            .entry(digest.clone())
                            .or_insert_with(|| owner.clone());
                    }
                    (None, url) => {
                        self.unchecked
                            .insert(url.clone().unwrap_or_else(|| owner.clone()));
                    }
                }
            }
            let downloads = !downloads.is_empty();
            if let Some(repo) = repo {
                if downloads {
                    self.downloaded.insert(repo.clone());
                }
                self.ran.insert(repo);
            }
        }
    }

    #[must_use]
    pub fn download_free(&self, repo: &str) -> bool {
        self.ran.contains(repo) && !self.downloaded.contains(repo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(number: u8, payload: &[u8]) -> Vec<u8> {
        let mut out = vec![
            (number << 3) | 2,
            u8::try_from(payload.len()).expect("short"),
        ];
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

    fn sha() -> String {
        "4e50948f".repeat(8)
    }

    #[test]
    fn reads_the_sorted_unique_contexts() {
        let with = |context: &[u8]| {
            let mut inner = field(1, b"location");
            inner.extend(field(2, context));
            let mut out = vec![u8::try_from(inner.len()).expect("short")];
            out.extend(inner);
            out
        };
        let mut log = with(b"@@x//:r");
        log.extend(with(b"@@a//:r"));
        log.extend(with(b"@@x//:r"));
        assert_eq!(contexts(&log), ["@@a//:r", "@@x//:r"]);
        assert_eq!(contexts(&[]), [] as [String; 0]);
    }

    #[test]
    fn reads_download_and_download_and_extract() {
        let mut first = field(1, b"https://a/x");
        first.extend(field(1, b"https://mirror/x"));
        first.extend(field(3, sha().as_bytes()));
        let mut second = field(1, b"https://b/y");
        second.extend(field(3, sha().as_bytes()));
        let mut log = event(4, &first);
        log.extend(event(5, &second));
        assert_eq!(
            downloads(&log),
            vec![
                ("https://a/x".to_owned(), sha()),
                ("https://b/y".to_owned(), sha()),
            ]
        );
    }

    #[test]
    fn falls_back_to_the_integrity_and_skips_unchecked_downloads() {
        let mut with_integrity = field(1, b"https://a/x");
        with_integrity.extend(field(
            5,
            ["sha256-Tk6Uj7uFZLijo7PzGzz", "+z9y+0mP2eoEw+32y+KHwEGM="]
                .concat()
                .as_bytes(),
        ));
        let unchecked = field(1, b"https://c/z");
        let mut log = event(4, &with_integrity);
        log.extend(event(4, &unchecked));
        let found = downloads(&log);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "https://a/x");
        assert_eq!(found[0].1.len(), 64);
    }

    #[test]
    fn tells_download_free_repositories_from_downloading_and_cached_ones() {
        let with = |context: &[u8], number: u8| {
            let mut inner = field(1, b"location");
            inner.extend(field(2, context));
            inner.extend(field(number, &field(1, b"file:///x")));
            let mut out = vec![u8::try_from(inner.len()).expect("short")];
            out.extend(inner);
            out
        };
        let mut log = with(b"repository @@+ext+hub", 8);
        log.extend(with(b"repository @@+ext+dl", 4));
        log.extend(with(b"repository @@+ext+dl", 8));
        log.extend(with(b"repository @@rules_x+", 5));
        log.extend(with(b"@@other//:r", 4));
        let mut fetches = Fetches::default();
        fetches.add(&log);
        assert_eq!(
            fetches.ran.iter().collect::<Vec<_>>(),
            ["+ext+dl", "+ext+hub", "rules_x+"]
        );
        assert_eq!(
            fetches.downloaded.iter().collect::<Vec<_>>(),
            ["+ext+dl", "rules_x+"]
        );
        assert!(fetches.download_free("+ext+hub"));
        assert!(!fetches.download_free("+ext+dl"));
        assert!(!fetches.download_free("cached+"));
    }

    #[test]
    fn keeps_each_archive_digest_with_its_first_fetcher_and_the_unchecked_urls() {
        let with = |context: &[u8], number: u8, message: &[u8]| {
            let mut inner = field(1, b"location");
            inner.extend(field(2, context));
            inner.extend(field(number, message));
            let mut out = vec![u8::try_from(inner.len()).expect("short")];
            out.extend(inner);
            out
        };
        let mut checked = field(1, b"https://a/x");
        checked.extend(field(3, sha().as_bytes()));
        let mut log = with(b"repository @@rules_x+", 5, &checked);
        log.extend(with(b"repository @@rules_y+", 4, &checked));
        log.extend(with(b"module extension ext", 4, &field(1, b"https://c/z")));
        let mut fetches = Fetches::default();
        fetches.add(&log);
        assert_eq!(
            fetches.archives.into_iter().collect::<Vec<_>>(),
            [(sha(), "rules_x+".to_owned())]
        );
        assert_eq!(
            fetches.unchecked.into_iter().collect::<Vec<_>>(),
            ["https://c/z"]
        );
        assert_eq!(
            fetches.downloaded.into_iter().collect::<Vec<_>>(),
            ["rules_x+", "rules_y+"]
        );
    }

    #[test]
    fn a_truncated_log_keeps_what_was_read() {
        let mut message = field(1, b"https://a/x");
        message.extend(field(3, sha().as_bytes()));
        let mut log = event(4, &message);
        log.extend([9, 1]);
        assert_eq!(downloads(&log).len(), 1);
    }
}
