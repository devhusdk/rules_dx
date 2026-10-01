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
        if !file.is_empty() {
            if let Some(reason) = documentation_ir::source_path_reason(file) {
                return Err(AdapterError::UnsafePath {
                    id: format!("markdown:{package}"),
                    path: (*file).to_owned(),
                    reason,
                });
            }
        }
    }
    shard("markdown", package, Vec::new())
}
