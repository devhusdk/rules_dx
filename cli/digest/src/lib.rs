#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::io;
use std::path::Path;

use sha2::{Digest as _, Sha256};

const FILE_CHUNK: usize = 64 << 10;

pub const DIGEST_LEN: usize = 32;

pub const SHA1_LEN: usize = 20;

pub type RawDigest = [u8; DIGEST_LEN];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DigestError {
    #[error("invalid digest {value:?}: want 64-character lowercase hex")]
    BadDigest { value: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Digest(pub RawDigest);

impl Digest {
    pub fn new(bytes: RawDigest) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &RawDigest {
        &self.0
    }

    pub fn into_bytes(self) -> RawDigest {
        self.0
    }

    pub fn blake3(bytes: &[u8]) -> Self {
        Self(blake3(bytes))
    }

    pub fn to_hex(&self) -> String {
        to_hex(&self.0)
    }

    pub fn parse_hex(text: &str) -> Result<Self, DigestError> {
        parse_hex(text).map(Self)
    }
}

impl From<RawDigest> for Digest {
    fn from(bytes: RawDigest) -> Self {
        Self(bytes)
    }
}

impl From<Digest> for RawDigest {
    fn from(digest: Digest) -> Self {
        digest.0
    }
}

pub fn blake3(bytes: &[u8]) -> RawDigest {
    *blake3::hash(bytes).as_bytes()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Hex sha256 of a file's bytes, read in fixed-size chunks.
pub fn sha256_file_hex(path: &Path) -> io::Result<String> {
    let mut hasher = Sha256::new();
    let mut file = std::fs::File::open(path)?;
    let mut buf = vec![0u8; FILE_CHUNK];
    loop {
        let read = io::Read::read(&mut file, &mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn is_hex(text: &str) -> bool {
    is_lower_hex(text, DIGEST_LEN)
}

pub fn is_lower_hex(text: &str, byte_len: usize) -> bool {
    match hex::decode(text) {
        Ok(bytes) => bytes.len() == byte_len && hex::encode(&bytes) == text,
        Err(_) => false,
    }
}

pub fn is_hex_any_case(text: &str, byte_len: usize) -> bool {
    match hex::decode(text) {
        Ok(bytes) => bytes.len() == byte_len,
        Err(_) => false,
    }
}

pub fn is_commit_sha(text: &str) -> bool {
    is_hex_any_case(text, SHA1_LEN) || is_hex_any_case(text, DIGEST_LEN)
}

pub fn is_sha256_hex(text: &str) -> bool {
    is_hex(text)
}

pub fn to_hex(bytes: &RawDigest) -> String {
    hex::encode(bytes)
}

pub fn to_hex_bytes(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

pub fn parse_hex(text: &str) -> Result<RawDigest, DigestError> {
    let bad = || DigestError::BadDigest {
        value: text.to_owned(),
    };
    let bytes = hex::decode(text).map_err(|_| bad())?;
    if bytes.len() != DIGEST_LEN || hex::encode(&bytes) != text {
        return Err(bad());
    }
    bytes.try_into().map_err(|_| bad())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn hex_round_trip_any_bytes(bytes in prop::array::uniform32(any::<u8>())) {
            let hex = to_hex(&bytes);
            prop_assert_eq!(hex.len(), 64);
            prop_assert!(is_hex(&hex));
            prop_assert!(is_sha256_hex(&hex));
            prop_assert_eq!(parse_hex(&hex), Ok(bytes));
            let wrapped = Digest::new(bytes);
            prop_assert_eq!(wrapped.to_hex(), hex.clone());
            prop_assert_eq!(Digest::parse_hex(&hex).unwrap().into_bytes(), bytes);
        }

        #[test]
        fn uppercase_rejected_when_letters_present(
            bytes in prop::array::uniform32(any::<u8>()),
        ) {
            let hex = to_hex(&bytes);
            prop_assume!(hex.chars().any(|c| c.is_ascii_alphabetic()));
            prop_assert!(!is_hex(&hex.to_uppercase()));
            prop_assert!(parse_hex(&hex.to_uppercase()).is_err());
        }

        #[test]
        fn mutated_encodings_rejected(
            bytes in prop::array::uniform32(any::<u8>()),
            idx in 0..64usize,
        ) {
            let hex = to_hex(&bytes);
            let extended = format!("{hex}00");
            prop_assert!(parse_hex(&extended).is_err());
            let mut chars: Vec<char> = hex.chars().collect();
            chars[idx] = 'z';
            let mutated: String = chars.into_iter().collect();
            prop_assert!(parse_hex(&mutated).is_err());
            prop_assert!(!is_hex(&mutated));
        }
    }

    #[test]
    fn blake3_empty_matches_official_vector() {
        let expected = [
            0xaf, 0x13, 0x49, 0xb9, 0xf5, 0xf9, 0xa1, 0xa6, 0xa0, 0x40, 0x4d, 0xea, 0x36, 0xdc,
            0xc9, 0x49, 0x9b, 0xcb, 0x25, 0xc9, 0xad, 0xc1, 0x12, 0xb7, 0xcc, 0x9a, 0x93, 0xca,
            0xe4, 0x1f, 0x32, 0x62,
        ];
        assert_eq!(blake3(b""), expected);
        assert_eq!(Digest::blake3(b"").into_bytes(), expected);
    }

    #[test]
    fn sha256_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_file_hex_matches_the_in_memory_digest() {
        let scratch = tempfile::tempdir().expect("scratch");
        for len in [
            0usize,
            1,
            FILE_CHUNK - 1,
            FILE_CHUNK,
            FILE_CHUNK + 1,
            3 * FILE_CHUNK,
        ] {
            let bytes: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
            let path = scratch.path().join(format!("payload-{len}.bin"));
            std::fs::write(&path, &bytes).expect("write");
            assert_eq!(
                sha256_file_hex(&path).expect("digest"),
                sha256_hex(&bytes),
                "len {len}"
            );
        }
        assert!(sha256_file_hex(&scratch.path().join("missing")).is_err());
    }

    #[test]
    fn hex_round_trip() {
        let bytes = blake3(b"hello");
        let hex = to_hex(&bytes);
        assert_eq!(hex.len(), 64);
        assert_eq!(parse_hex(&hex).expect("round trip"), bytes);
        assert_eq!(
            Digest::parse_hex(&hex).expect("wrapper").into_bytes(),
            bytes
        );
        assert_eq!(Digest::new(bytes).to_hex(), hex);
    }

    #[test]
    fn hex_spellings_rejected() {
        let good = to_hex(&blake3(b"x"));
        assert!(is_hex(&good));
        assert!(is_sha256_hex(&good));
        assert!(parse_hex(&good).is_ok());
        assert!(parse_hex(&good.to_uppercase()).is_err());
        assert!(!is_hex(&good.to_uppercase()));
        assert!(parse_hex("0123").is_err());
        assert!(parse_hex(&format!("{good}00")).is_err());
        assert!(parse_hex(&"zz".repeat(32)).is_err());
        assert_eq!(
            parse_hex("xyz"),
            Err(DigestError::BadDigest {
                value: "xyz".to_owned()
            })
        );
    }

    #[test]
    fn display_is_human_readable() {
        let rendered = format!(
            "{}",
            DigestError::BadDigest {
                value: "xyz".to_owned()
            }
        );
        assert!(rendered.contains("64-character lowercase hex"));
    }

    #[test]
    fn commit_sha_accepts_40_and_64_in_either_case() {
        let sha40 = "3d3c42e5aac5ba805825da76410c181273ba90b1";
        let sha64 = to_hex(&blake3(b"commit"));
        assert!(is_commit_sha(sha40));
        assert!(is_commit_sha(&sha40.to_uppercase()));
        assert!(is_commit_sha(&sha64));
        assert!(is_commit_sha(&sha64.to_uppercase()));
        assert!(!is_commit_sha("3d3c42e5"));
        assert!(!is_commit_sha("v4"));
        assert!(!is_commit_sha(""));
        assert!(!is_commit_sha(&format!("{sha40}00")));
        assert!(!is_commit_sha(&"zz".repeat(20)));
        assert!(is_hex_any_case(sha40, SHA1_LEN));
        assert!(is_hex_any_case(&sha40.to_uppercase(), SHA1_LEN));
        assert!(!is_hex_any_case(sha40, DIGEST_LEN));
        assert!(is_hex_any_case(&sha64, DIGEST_LEN));
    }

    #[test]
    fn lower_hex_enforces_length_and_case() {
        let sha40 = "3d3c42e5aac5ba805825da76410c181273ba90b1";
        assert!(is_lower_hex(sha40, SHA1_LEN));
        assert!(!is_lower_hex(&sha40.to_uppercase(), SHA1_LEN));
        assert!(!is_lower_hex("3d3c42e5", SHA1_LEN));
        assert!(!is_lower_hex("v7", SHA1_LEN));
        assert!(!is_lower_hex("", SHA1_LEN));
        assert!(!is_lower_hex(&to_hex(&blake3(b"pin")), SHA1_LEN));
        let digest_hex = to_hex(&blake3(b"x"));
        assert!(is_lower_hex(&digest_hex, DIGEST_LEN));
        assert_eq!(is_hex(&digest_hex), is_lower_hex(&digest_hex, DIGEST_LEN));
    }
}
