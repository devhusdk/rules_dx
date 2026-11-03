//! Builds the Bazel flags CI passes for a remote cache.

/// What the caller configured.
pub struct Cache<'a> {
    pub url: &'a str,
    pub header: &'a str,
    pub buildbuddy_key: &'a str,
    pub event: &'a str,
}

/// Returns one Bazel flag per entry, or an error for a bad header.
pub fn flags(cache: &Cache) -> Result<Vec<String>, String> {
    if cache.url.is_empty() && cache.buildbuddy_key.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = vec!["--config=ci".to_string()];
    if cache.event == "pull_request" {
        out.push("--config=ci-pr".to_string());
    }
    if !cache.url.is_empty() {
        out.push(format!("--remote_cache={}", cache.url));
    }
    if !cache.header.is_empty() {
        match cache.header.split_once('=') {
            Some((name, value)) if !name.is_empty() && !value.is_empty() => {}
            _ => return Err("REMOTE_CACHE_HEADER must look like name=value".to_string()),
        }
        out.push(format!("--remote_header={}", cache.header));
    } else if !cache.buildbuddy_key.is_empty() {
        out.push(format!(
            "--remote_header=x-buildbuddy-api-key={}",
            cache.buildbuddy_key
        ));
    }
    Ok(out)
}
