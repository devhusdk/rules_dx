use super::*;

fn config() -> Config {
    serde_json::from_str(
        r#"{
            "loader": "dx_closure/lib/ld-musl-x86_64.so.1",
            "library_path": ["dx_closure/lib", "lib/x86_64-linux-ghc-9.10.3"],
            "tools": {
                "ghc": ["-B{root}/lib"],
                "hpc": [],
                "runghc": ["-f", "{bin}/ghc"]
            }
        }"#,
    )
    .expect("config")
}

#[test]
fn an_execroot_wrapper_finds_the_repository_source_tree() {
    let dir = Path::new(
        "/x/execroot/_main/bazel-out/k8-opt-exec-ST/bin/external/+haskell+ghc/musl_tools",
    );
    assert_eq!(
        repo_root(dir),
        Ok(PathBuf::from("/x/execroot/_main/external/+haskell+ghc"))
    );
}

#[test]
fn a_runfiles_wrapper_finds_the_repository_next_to_it() {
    let dir = Path::new("/r/lock.runfiles/+haskell+ghc/musl_tools");
    assert_eq!(
        repo_root(dir),
        Ok(PathBuf::from("/r/lock.runfiles/+haskell+ghc"))
    );
}

#[test]
fn a_tool_runs_its_bindist_binary_under_the_loader_with_its_extra_args() {
    let root = Path::new("/e/external/ghc");
    let planned = plan(
        Path::new("/w/musl_tools/ghc"),
        &config(),
        root,
        vec![OsString::from("--numeric-version")],
    )
    .expect("plan");
    assert_eq!(
        planned,
        Plan {
            loader: PathBuf::from("/e/external/ghc/dx_closure/lib/ld-musl-x86_64.so.1"),
            args: [
                "--library-path",
                "/e/external/ghc/dx_closure/lib:/e/external/ghc/lib/x86_64-linux-ghc-9.10.3",
                "/e/external/ghc/bin/ghc",
                "-B/e/external/ghc/lib",
                "--numeric-version",
            ]
            .map(OsString::from)
            .to_vec(),
        }
    );
}

#[test]
fn bin_names_the_wrapper_directory() {
    let planned = plan(
        Path::new("/w/musl_tools/runghc"),
        &config(),
        Path::new("/e"),
        Vec::new(),
    )
    .expect("plan");
    assert_eq!(
        planned.args.get(3..),
        Some([OsString::from("-f"), OsString::from("/w/musl_tools/ghc")].as_slice())
    );
}

#[test]
fn a_tool_missing_from_the_config_fails() {
    let error = plan(
        Path::new("/w/musl_tools/haddock"),
        &config(),
        Path::new("/e"),
        Vec::new(),
    )
    .unwrap_err();
    assert_eq!(error, "dx_ghc_musl.json has no tool haddock");
}

#[test]
fn a_bare_program_name_fails() {
    assert_eq!(run_tool(OsStr::new("ghc"), Vec::new()), 1);
}
