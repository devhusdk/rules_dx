use qt_board_support::*;

#[test]
fn native_display_renders_the_relocated_board() {
    let displays =
        std::env::var("DX_QT_DISPLAYS").expect("DX_QT_DISPLAYS names xcb, wayland or both");
    assert!(!displays.is_empty(), "DX_QT_DISPLAYS is empty");
    let consumer = Consumer::new("display", "DX_QT_TASKS_MANIFEST", None);
    assert_exit(
        &consumer.dx(
            "build",
            &[
                "//examples/qt-tasks/app:board_release_archive",
                "//examples/qt-tasks/app:board_release_checksum",
            ],
            &[],
        ),
        0,
    );
    let board_dir = extract(&consumer, "examples/qt-tasks/app", "board_release");
    std::fs::remove_dir_all(&consumer.root).expect("source tree removed");
    let board = board_dir.join("board");
    let store = board_dir.join("tasks.json");
    write(
        &store,
        "[{\"title\": \"Draw logo\", \"asset\": \"logo\", \"done\": true},\n {\"title\": \"Cut trailer\", \"asset\": \"intro\", \"done\": false}]\n",
    );
    let store = store.to_string_lossy().into_owned();
    for display in displays.split(',') {
        let (platform, variables): (&str, &[&str]) = match display {
            "xcb" => ("xcb", &["DISPLAY", "XAUTHORITY"]),
            "wayland" => ("wayland", &["WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"]),
            other => panic!("unknown display {other:?}"),
        };
        let values: Vec<(String, String)> = variables
            .iter()
            .filter_map(|name| {
                std::env::var(name)
                    .ok()
                    .map(|value| (name.to_string(), value))
            })
            .collect();
        assert!(
            values.iter().any(|(name, _)| name == variables[0]),
            "{display}: {} is not set, so there is no display to qualify",
            variables[0]
        );
        let image = format!("{display}.ppm");
        let mut env: Vec<(&str, &str)> = values
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        env.extend([("QT_QPA_PLATFORM", platform), ("LD_DEBUG", "libs")]);
        let rendered = launch_with(&board, &["--store", &store, &image], &env);
        assert_exit(&rendered, 0);
        assert!(
            rendered
                .stdout
                .ends_with(&format!("{image} 320x72 {platform}\n")),
            "{}",
            rendered.stdout
        );
        assert_board_pixels(&std::fs::read(board_dir.join(&image)).expect("image written"));
        let plugin = format!("platforms/libq{platform}.so");
        assert_bundled_only(&rendered.stderr, &board_dir, &plugin);
    }
}
