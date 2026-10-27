pub use dx_preset::{
    bazelrc_imports, import_line, layer_import_state, owned_collisions_in_content, preset_paths,
    render_all, render_layer, ImportState, Layer, CONSUMER_DIR, HEADER_MARKER,
    PRESET_BAZEL_VERSION, REPO_DIR,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dx_stamp_tracks_single_version() {
        assert_eq!(
            crate::version::DX_VERSION,
            crate::version::repo_module_version()
        );
        assert_eq!(
            PRESET_BAZEL_VERSION,
            crate::version::repo_text(".bazelversion").trim()
        );
    }
}
