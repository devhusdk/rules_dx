use documentation_ir::proto::DocIr;

use crate::common::shard;
use crate::types::AdapterError;

pub fn confirm_prose_only(package: &str, files: &[&str]) -> Result<DocIr, AdapterError> {
    if package.is_empty() {
        return Err(AdapterError::EmptyIdentity);
    }
    for file in files {
        if !(file.ends_with(".md")
            || file.ends_with(".mdx")
            || file.ends_with(".astro")
            || file.is_empty())
        {
            return Err(AdapterError::NonMarkdown((*file).to_owned()));
        }
        if file.starts_with('/') {
            return Err(AdapterError::AbsolutePath {
                id: format!("markdown:{package}"),
                path: (*file).to_owned(),
            });
        }
    }
    shard("markdown", package, Vec::new())
}
