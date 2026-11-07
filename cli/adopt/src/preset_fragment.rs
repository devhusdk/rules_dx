pub use dx_preset::{
    CONSUMER_DIR, HEADER_MARKER, ImportState, Layer, PRESET_BAZEL_VERSION, bazelrc_imports,
    collision_lines, import_line, is_generated, layer_import_state, layer_is_current,
    layer_is_current_with, layer_warnings_as_errors, owned_collisions_in_content, prepend_imports,
    preset_paths, render_all, render_layer, render_layer_with, rendered_flag_lines,
};

#[cfg(test)]
mod tests {
    #[test]
    fn dx_stamp_tracks_single_version() {
        assert_eq!(
            crate::version::DX_VERSION,
            crate::version::repo_module_version()
        );
    }
}
