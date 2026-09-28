#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct FileVerdict {
    pub path: String,
    pub covered: u64,
    pub eligible: u64,
    pub uncovered: Vec<u32>,
    pub ignored: u64,
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct GateVerdict {
    pub files: Vec<FileVerdict>,
    pub covered: u64,
    pub eligible: u64,
    pub passed: bool,
    pub errors: Vec<String>,
    pub other_sources: Vec<String>,
}

#[path = "verdict_eval.rs"]
mod verdict_eval;
#[path = "verdict_lang.rs"]
mod verdict_lang;
#[path = "verdict_render.rs"]
mod verdict_render;

pub use verdict_eval::*;
pub use verdict_lang::*;
pub use verdict_render::*;

#[cfg(test)]
#[path = "verdict_tests.rs"]
mod verdict_tests;
