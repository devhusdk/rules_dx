use std::path::{Path, PathBuf};

use hermetic::{
    ambient_tools, capture, preflight, Isolation, Net, AMBIENT_MARKER, AMBIENT_TOOLS, SHIM,
    TOOL_DIRS,
};

fn scratch() -> PathBuf {
    let tmp = PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR is set under Bazel"));
    let root = tmp.join("isolation probe");
    hermetic::remove_root(&root);
    std::fs::create_dir_all(&root).expect("scratch root");
    root
}

fn inside(isolation: &Isolation, net: Net, script: &str) -> hermetic::Outcome {
    let argv = vec!["/bin/bash".to_owned(), "-c".to_owned(), script.to_owned()];
    capture(isolation.command(net, &argv)).expect("isolated shell starts")
}

#[test]
fn isolation_denies_network_masks_tools_and_config_and_keeps_the_real_user() {
    use std::os::unix::fs::PermissionsExt;
    let root = scratch();
    let shim = root.join("ambient-tool");
    std::fs::write(&shim, SHIM).expect("shim");
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).expect("shim mode");
    let home = root.join("real home");
    std::fs::create_dir_all(home.join(".cargo/bin")).expect("cargo home");
    std::fs::write(home.join(".cargo/config.toml"), "[build]\n").expect("cargo config");
    std::fs::write(home.join(".netrc"), "machine example.com login me\n").expect("netrc");
    let isolation = Isolation::plan(&shim, std::slice::from_ref(&shim), Some(&home)).expect("plan");
    assert_eq!(isolation.home_dirs, vec![home.join(".cargo")]);
    assert_eq!(isolation.home_files, vec![home.join(".netrc")]);
    preflight(&isolation).expect("namespaces are available");
    let dirs: Vec<PathBuf> = TOOL_DIRS.iter().map(PathBuf::from).collect();
    let (tool, _) = ambient_tools(&dirs, AMBIENT_TOOLS)
        .into_iter()
        .find(|(_, path)| isolation.tools.contains(path))
        .expect("this host has an ambient tool to mask");
    let home_text = home.to_string_lossy();
    let script = format!(
        "(exec 3<>/dev/tcp/1.1.1.1/443) && echo network=open\n\
         echo uid=$(id -u)\n\
         echo devices=$(tail -n +3 /proc/net/dev | cut -d: -f1 | tr -d ' ' | tr '\\n' ,)\n\
         echo cargo=$(ls -A '{home_text}/.cargo' | wc -l)\n\
         echo netrc=$(wc -c < '{home_text}/.netrc')\n\
         echo sdk=$(find {sdk} -mindepth 1 | wc -l)\n\
         {tool} --version\n\
         echo tool_exit=$?\n",
        sdk = isolation
            .sdk_dirs
            .iter()
            .map(|dir| format!("'{}'", dir.display()))
            .collect::<Vec<_>>()
            .join(" "),
    );
    let denied = inside(&isolation, Net::Denied, &script);
    assert_eq!(denied.exit, 0, "{denied:?}");
    let uid = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| hermetic::status_id(&status, "Uid:"))
        .expect("own uid");
    for line in [
        format!("uid={uid}"),
        "devices=lo,".to_owned(),
        "cargo=0".to_owned(),
        "netrc=0".to_owned(),
        "tool_exit=97".to_owned(),
    ] {
        assert!(
            denied.stdout.lines().any(|out| out == line),
            "missing {line}: {denied:?}"
        );
    }
    if !isolation.sdk_dirs.is_empty() {
        assert!(
            denied.stdout.lines().any(|out| out == "sdk=0"),
            "{denied:?}"
        );
    }
    assert!(!denied.stdout.contains("network=open"), "{denied:?}");
    assert!(
        denied.stderr.contains("Network is unreachable"),
        "{denied:?}"
    );
    assert!(
        denied.stderr.contains(&format!("{AMBIENT_MARKER} ")),
        "{denied:?}"
    );
    let allowed = inside(
        &isolation,
        Net::Allowed,
        "tail -n +3 /proc/net/dev | cut -d: -f1 | tr -d ' ' | tr '\\n' ,",
    );
    assert_eq!(allowed.exit, 0, "{allowed:?}");
    let host = std::fs::read_to_string("/proc/net/dev").expect("host devices");
    let expected: String = host
        .lines()
        .skip(2)
        .filter_map(|line| line.split(':').next())
        .map(|name| format!("{},", name.trim()))
        .collect();
    assert_eq!(allowed.stdout, expected, "fetch keeps the host network");
    assert!(
        Path::new(&home)
            .join(".netrc")
            .metadata()
            .expect("netrc")
            .len()
            > 0
    );
}
