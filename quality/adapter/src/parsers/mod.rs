pub mod biome;
pub mod buf;
pub mod buildifier;
pub mod checkstyle;
pub mod clang_format;
pub mod clang_tidy;
pub mod cppcheck;
pub mod csharpier;
pub mod cue;
pub mod djlint;
pub mod errcheck;
pub mod error_prone;
pub mod eslint;
pub mod fantomas;
pub mod flake8;
pub mod fsharplint;
pub mod gofumpt;
pub mod google_java_format;
pub mod govet;
pub mod jsonnetfmt;
pub mod keep_sorted;
pub mod ktfmt;
pub mod ktlint;
pub mod markdown;
pub mod modfmt;
pub mod pkl;
pub mod pmd;
pub mod prettier;
pub mod psscriptanalyzer;
pub mod pydoclint;
pub mod pylint;
pub mod qmlformat;
pub mod qmllint;
pub mod roslyn;
pub mod rubocop;
pub mod ruff;
pub mod rust;
pub mod rustfmt;
pub mod sarif;
pub mod scalafix;
pub mod scalafmt;
pub mod shellcheck;
pub mod shfmt;
pub mod spotbugs;
pub mod standardrb;
pub mod staticcheck;
pub mod stylelint;
pub mod taplo;
pub mod terraform;
pub mod tsc;
pub mod ty;
pub mod vale;
pub mod yamlfmt;
pub mod yamllint;

pub use biome::{parse_biome_format, parse_biome_lint};
pub use buf::{parse_buf_format, parse_buf_lint};
pub use buildifier::parse_buildifier;
pub use checkstyle::parse_checkstyle;
pub use clang_format::parse_clang_format;
pub use clang_tidy::parse_clang_tidy;
pub use cppcheck::parse_cppcheck;
pub use csharpier::parse_csharpier;
pub use cue::parse_cue;
pub use djlint::{parse_djlint, parse_djlint_format};
pub use errcheck::parse_errcheck;
pub use error_prone::parse_error_prone;
pub use eslint::parse_eslint;
pub use fantomas::parse_fantomas;
pub use flake8::parse_flake8;
pub use fsharplint::parse_fsharplint;
pub use gofumpt::parse_gofumpt;
pub use google_java_format::parse_google_java_format;
pub use govet::parse_govet;
pub use jsonnetfmt::parse_jsonnetfmt;
pub use keep_sorted::parse_keep_sorted;
pub use ktfmt::parse_ktfmt;
pub use ktlint::parse_ktlint;
pub use markdown::parse_markdown_findings;
pub use modfmt::parse_modfmt;
pub use pkl::parse_pkl;
pub use pmd::parse_pmd;
pub use prettier::parse_prettier_check;
pub use psscriptanalyzer::parse_psscriptanalyzer;
pub use pydoclint::parse_pydoclint;
pub use pylint::parse_pylint;
pub use qmlformat::parse_qmlformat;
pub use qmllint::parse_qmllint;
pub use roslyn::parse_roslyn;
pub use rubocop::parse_rubocop;
pub use ruff::{parse_ruff, parse_ruff_format};
pub use rust::{parse_clippy, parse_rustc};
pub use rustfmt::parse_rustfmt;
pub use scalafix::parse_scalafix;
pub use scalafmt::parse_scalafmt;
pub use shellcheck::parse_shellcheck;
pub use shfmt::parse_shfmt;
pub use spotbugs::parse_spotbugs;
pub use standardrb::parse_standardrb;
pub use staticcheck::parse_staticcheck;
pub use stylelint::parse_stylelint;
pub use taplo::{parse_taplo_format_check, parse_taplo_lint};
pub use terraform::parse_terraform;
pub use tsc::parse_tsc;
pub use ty::parse_ty;
pub use vale::parse_vale;
pub use yamlfmt::parse_yamlfmt;
pub use yamllint::parse_yamllint;

use crate::{Finding, TextPosition};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFinding {
    pub file: String,
    pub finding: Finding,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("{tool} output is not the pinned JSON grammar: {detail}")]
    Json { tool: &'static str, detail: String },
    #[error("{tool} output is outside the pinned grammar: {detail}")]
    Shape { tool: &'static str, detail: String },
    #[error("{tool} reported an unchecked file: {path}")]
    UnknownFile { tool: &'static str, path: String },
    #[error("vale needs a usable config: {detail}")]
    ValeConfig { detail: String },
    #[error("{tool} output exceeds max size {limit} bytes (got {bytes})")]
    TooLarge {
        tool: &'static str,
        bytes: usize,
        limit: usize,
    },
}

pub const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

pub fn check_output_size(tool: &'static str, bytes: &[u8]) -> Result<(), ParseError> {
    if bytes.len() > MAX_OUTPUT_BYTES {
        return Err(ParseError::TooLarge {
            tool,
            bytes: bytes.len(),
            limit: MAX_OUTPUT_BYTES,
        });
    }
    Ok(())
}

fn known<'a>(tool: &'static str, files: &[&'a str], path: &str) -> Result<&'a str, ParseError> {
    files
        .iter()
        .find(|file| **file == path)
        .copied()
        .ok_or_else(|| ParseError::UnknownFile {
            tool,
            path: path.to_owned(),
        })
}

fn point(line: u64, column: u64) -> (TextPosition, Option<TextPosition>) {
    (TextPosition { line, column }, None)
}

fn missing(tool: &'static str, what: &str, line: &str) -> ParseError {
    ParseError::Shape {
        tool,
        detail: format!("malformed {what}: {line}"),
    }
}

fn code_name(code: Option<i32>) -> String {
    code.map_or_else(|| "signal".to_owned(), |code| code.to_string())
}

#[cfg(test)]
mod tests {
    use super::{check_output_size, ParseError, MAX_OUTPUT_BYTES};

    #[test]
    fn error_display_is_stable() {
        assert!(ParseError::Json {
            tool: "vale",
            detail: "x".to_owned()
        }
        .to_string()
        .contains("vale"));
        assert!(ParseError::UnknownFile {
            tool: "taplo",
            path: "/s/x".to_owned()
        }
        .to_string()
        .contains("/s/x"));
        assert!(ParseError::ValeConfig {
            detail: "E100".to_owned()
        }
        .to_string()
        .contains("E100"));
        assert!(ParseError::TooLarge {
            tool: "sarif",
            bytes: MAX_OUTPUT_BYTES + 1,
            limit: MAX_OUTPUT_BYTES,
        }
        .to_string()
        .contains("max size"));
    }

    #[test]
    fn output_size_guard_rejects_oversized() {
        assert!(check_output_size("tsc", b"ok").is_ok());
        assert!(check_output_size("tsc", &vec![b'x'; MAX_OUTPUT_BYTES]).is_ok());
        let big = vec![b'x'; MAX_OUTPUT_BYTES + 1];
        assert_eq!(
            check_output_size("tsc", &big),
            Err(ParseError::TooLarge {
                tool: "tsc",
                bytes: big.len(),
                limit: MAX_OUTPUT_BYTES,
            })
        );
        assert!(super::tsc::parse_tsc(&big, Some(2), &["/s/a.ts"]).is_err());
        assert!(super::sarif::parse_sarif("sarif-test", &big, Some(1), &["/s/a.java"]).is_err());
    }

    fn xorshift(state: &mut u64) -> u64 {
        let mut x = *state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *state = x;
        x
    }

    #[test]
    fn fuzz_parsers_never_panic_on_arbitrary_bytes() {
        let seeds: &[&[u8]] = &[
            b"{}",
            b"[]",
            b"not json",
            b"/s/a.ts(1,1): error TS1234: msg\n",
            br#"{"version":"2.1.0","runs":[]}"#,
            br#"[{"filePath":"/s/a.js","messages":[{"ruleId":"x","severity":2,"message":"m","line":1,"column":1}]}]"#,
            br#"{"success":false,"files":[{"filename":"/s/a.bzl","formatted":false,"valid":true,"warnings":[]}]}"#,
            b"path:1:1: E100 message\n",
            b"error: bad\n  \xe2\x94\x8c\xe2\x94\x80 /s/x.toml:1:5\n",
            b"Diff in /s/x.rs:1:\n-fn  main(){}\n+fn main() {}\n",
            b"--- a/x.py\n+++ b/x.py\n@@ -1 +1 @@\n-a\n+b\n",
            b"{\"path\":\"x.proto\",\"start_line\":1,\"start_column\":1,\"type\":\"T\",\"message\":\"m\"}\n",
            b"/s/Hello.java:3: error: [DeadException] msg\n1 error\n",
            b"<results version=\"2\"><errors><error id=\"a\" severity=\"error\" msg=\"m\"><location file=\"/s/a.c\" line=\"1\"/></error></errors></results>",
            b"\xff\xfe\x00",
        ];
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        for round in 0..400 {
            let seed = seeds[round % seeds.len()];
            let mut input = seed.to_vec();
            match xorshift(&mut state) % 3 {
                0 => {
                    let keep = (xorshift(&mut state) as usize) % (input.len() + 1);
                    input.truncate(keep);
                }
                1 => {
                    if !input.is_empty() {
                        let at = (xorshift(&mut state) as usize) % input.len();
                        input[at] ^= (xorshift(&mut state) & 0xFF) as u8;
                    }
                }
                _ => {
                    let extra = (xorshift(&mut state) % 32) as usize;
                    for _ in 0..extra {
                        input.push((xorshift(&mut state) & 0xFF) as u8);
                    }
                }
            }
            let files = [
                "/s/a.ts",
                "/s/a.java",
                "/s/a.js",
                "/s/a.bzl",
                "/s/x.toml",
                "/s/x.rs",
                "/s/a.py",
                "/s/x.proto",
                "/s/Hello.java",
                "/s/dirty.toml",
            ];
            let _ = super::tsc::parse_tsc(&input, Some(2), &files);
            let _ = super::sarif::parse_sarif("fuzz", &input, Some(1), &files);
            let _ = super::ruff::parse_ruff(&input, Some(1), &files);
            let _ = super::ruff::parse_ruff_format(&input, Some(1), &files);
            let _ = super::vale::parse_vale(&input, Some(1), &files);
            let _ = super::rust::parse_clippy(&input, Some(1), &files);
            let _ = super::rust::parse_rustc(&input, Some(1), &files);
            let _ = super::buildifier::parse_buildifier(&input, &input, &files);
            let _ = super::eslint::parse_eslint(&input, Some(1), &files);
            let _ = super::ty::parse_ty(&input, Some(1), &files);
            let _ = super::taplo::parse_taplo_lint(&input, Some(1), &files);
            let _ = super::taplo::parse_taplo_format_check(&input, Some(1), &files);
            let _ = super::rustfmt::parse_rustfmt(&input, &input, Some(1), &files);
            let _ = super::error_prone::parse_error_prone(&input, &input, Some(1), &files);
            let _ = super::buf::parse_buf_lint(&input, Some(1), &files);
            let _ = super::buf::parse_buf_format(&input, Some(1), &files);
            let _ = super::djlint::parse_djlint(&input, Some(1), &files);
            let _ = super::djlint::parse_djlint_format(&input, Some(1), &files);
            let _ = super::shellcheck::parse_shellcheck(&input, Some(1), &files);
            let _ = super::prettier::parse_prettier_check(&input, Some(1), &files);
            let _ = super::govet::parse_govet(&input, Some(1), &files);
            let _ = super::gofumpt::parse_gofumpt(&input, Some(1), &files);
            let _ = super::markdown::parse_markdown_findings(&input, Some(0), &files);
            let _ = super::spotbugs::parse_spotbugs(&input, Some(1), &files);
            let _ = super::cppcheck::parse_cppcheck(&input, Some(1), &files);
        }
    }
}
