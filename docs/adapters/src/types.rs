#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AdapterError {
    #[error("empty native input")]
    EmptyInput,
    #[error("pinned-producer mismatch: expected {expected}, found {found}")]
    VersionMismatch { expected: String, found: String },
    #[error("native JSON does not parse: {0}")]
    InvalidJson(String),
    #[error("empty language or package")]
    EmptyIdentity,
    #[error("unsafe source path rejected for {id}: {path}: {reason}")]
    UnsafePath {
        id: String,
        path: String,
        reason: &'static str,
    },
    #[error("duplicate symbol id: {0}")]
    DuplicateId(String),
    #[error("missing or incompatible TASTy: empty inputs never inventory")]
    MissingTasty,
    #[error("prose-only package saw a non-markdown file: {0}")]
    NonMarkdown(String),
    #[error("IR rejected by the versioned codec: {0}")]
    InvalidIr(String),
}
