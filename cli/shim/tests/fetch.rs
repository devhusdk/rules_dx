mod support;

use std::path::{Path, PathBuf};

use dx_shim::fetch::{self, ChecksumSource, Request};
use support::{leftovers, Reply, Server};

const VERSION: &str = "1.2.3";
const ASSET: &str = "dx-linux-x86_64";

fn install(url: &str, dest: &Path, checksum: &ChecksumSource) -> Result<PathBuf, String> {
    fetch::install(&Request {
        version: VERSION,
        asset: ASSET,
        url,
        dest,
        checksum,
    })
    .map_err(|e| e.to_string())
}

fn ensure(url: &str, dest: &Path) -> Result<PathBuf, String> {
    fetch::ensure(&Request {
        version: VERSION,
        asset: ASSET,
        url,
        dest,
        checksum: &ChecksumSource::Sidecar,
    })
    .map_err(|e| e.to_string())
}

fn dest(cache: &tempfile::TempDir) -> PathBuf {
    cache.path().join("bin").join(VERSION).join(ASSET)
}

#[test]
fn good_binary_installs() {
    let server = Server::start();
    server.release(VERSION, ASSET, b"binary", b"binary");
    let cache = tempfile::tempdir().expect("tempdir");
    let dest = dest(&cache);
    let url = server.url(VERSION, ASSET);
    assert_eq!(
        install(&url, &dest, &ChecksumSource::Sidecar).expect("installs"),
        dest
    );
    assert_eq!(std::fs::read(&dest).expect("read"), b"binary");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&dest).expect("meta").permissions().mode();
        assert_eq!(mode & 0o777, 0o755);
    }
    assert!(leftovers(dest.parent().expect("dir")).is_empty());
}

#[test]
fn uppercase_sidecar_is_accepted() {
    let server = Server::start();
    server.route(&format!("/v{VERSION}/{ASSET}"), Reply::Ok(b"bin".to_vec()));
    server.route(
        &format!("/v{VERSION}/{ASSET}.sha256"),
        Reply::Ok(support::sha256_hex(b"bin").to_uppercase().into_bytes()),
    );
    let cache = tempfile::tempdir().expect("tempdir");
    let dest = dest(&cache);
    install(&server.url(VERSION, ASSET), &dest, &ChecksumSource::Sidecar).expect("installs");
}

#[test]
fn wrong_sidecar_is_refused_and_leaves_nothing() {
    let server = Server::start();
    server.release(VERSION, ASSET, b"evil", b"binary");
    let cache = tempfile::tempdir().expect("tempdir");
    let dest = dest(&cache);
    let url = server.url(VERSION, ASSET);
    let err = install(&url, &dest, &ChecksumSource::Sidecar).expect_err("refused");
    assert!(
        err.starts_with(&format!("dx: sha256 mismatch for {url}: want '")),
        "{err}"
    );
    assert!(err.ends_with("refusing to run it"), "{err}");
    assert!(!dest.exists());
    assert!(leftovers(dest.parent().expect("dir")).is_empty());
}

#[test]
fn garbage_sidecar_is_refused() {
    let server = Server::start();
    server.route(&format!("/v{VERSION}/{ASSET}"), Reply::Ok(b"bin".to_vec()));
    server.route(
        &format!("/v{VERSION}/{ASSET}.sha256"),
        Reply::Ok(b"not-a-hash dx\n".to_vec()),
    );
    let cache = tempfile::tempdir().expect("tempdir");
    let dest = dest(&cache);
    install(&server.url(VERSION, ASSET), &dest, &ChecksumSource::Sidecar).expect_err("refused");
    assert!(!dest.exists());
}

#[test]
fn oversized_sidecar_is_refused() {
    let server = Server::start();
    server.route(&format!("/v{VERSION}/{ASSET}"), Reply::Ok(b"bin".to_vec()));
    let mut big = support::sha256_hex(b"bin").into_bytes();
    big.resize(64 * 1024, b' ');
    server.route(&format!("/v{VERSION}/{ASSET}.sha256"), Reply::Ok(big));
    let cache = tempfile::tempdir().expect("tempdir");
    let dest = dest(&cache);
    install(&server.url(VERSION, ASSET), &dest, &ChecksumSource::Sidecar).expect_err("refused");
    assert!(!dest.exists());
}

#[test]
fn pinned_hash_is_used_without_the_sidecar() {
    let server = Server::start();
    server.release(VERSION, ASSET, b"binary", b"other");
    let cache = tempfile::tempdir().expect("tempdir");
    let url = server.url(VERSION, ASSET);
    let pinned = ChecksumSource::Pinned(support::sha256_hex(b"binary"));
    install(&url, &dest(&cache), &pinned).expect("pinned wins over the sidecar");
    let other = tempfile::tempdir().expect("tempdir");
    let wrong = ChecksumSource::Pinned(support::sha256_hex(b"nope"));
    install(&url, &dest(&other), &wrong).expect_err("pinned mismatch");
    install(&url, &dest(&other), &ChecksumSource::Pinned("short".into())).expect_err("bad pin");
    assert!(!dest(&other).exists());
}

#[test]
#[ignore = "needs the pinned checksum source as the default"]
fn swapped_binary_with_matching_sidecar_is_refused() {
    let server = Server::start();
    server.release(VERSION, ASSET, b"swapped", b"swapped");
    let cache = tempfile::tempdir().expect("tempdir");
    let dest = dest(&cache);
    ensure(&server.url(VERSION, ASSET), &dest).expect_err("swapped binary must be refused");
    assert!(!dest.exists());
}

#[test]
fn truncated_body_is_refused() {
    let server = Server::start();
    server.route(
        &format!("/v{VERSION}/{ASSET}"),
        Reply::Truncated(b"bin".to_vec(), 1000),
    );
    server.route(
        &format!("/v{VERSION}/{ASSET}.sha256"),
        Reply::Ok(support::sha256_hex(b"bin").into_bytes()),
    );
    let cache = tempfile::tempdir().expect("tempdir");
    let dest = dest(&cache);
    let url = server.url(VERSION, ASSET);
    let err = install(&url, &dest, &ChecksumSource::Sidecar).expect_err("truncated");
    assert!(
        err.starts_with(&format!("dx: download failed: {url}")),
        "{err}"
    );
    assert!(!dest.exists());
    assert!(leftovers(dest.parent().expect("dir")).is_empty());
}

#[test]
fn not_found_names_version_and_url() {
    let server = Server::start();
    let cache = tempfile::tempdir().expect("tempdir");
    let url = server.url(VERSION, ASSET);
    let err = install(&url, &dest(&cache), &ChecksumSource::Sidecar).expect_err("404");
    assert_eq!(
        err,
        format!("dx: dx {VERSION} has no release asset {ASSET} (HTTP 404): {url}")
    );
}

#[test]
fn network_failure_names_the_url() {
    let cache = tempfile::tempdir().expect("tempdir");
    let url = format!("{}/v{VERSION}/{ASSET}", support::dead_base());
    let err = ensure(&url, &dest(&cache)).expect_err("refused");
    assert!(
        err.starts_with(&format!("dx: download failed: {url}: ")),
        "{err}"
    );
}

#[test]
fn cached_binary_needs_no_network() {
    let cache = tempfile::tempdir().expect("tempdir");
    let dest = dest(&cache);
    std::fs::create_dir_all(dest.parent().expect("dir")).expect("mkdir");
    std::fs::write(&dest, b"cached").expect("seed");
    let url = format!("{}/v{VERSION}/{ASSET}", support::dead_base());
    assert_eq!(ensure(&url, &dest).expect("cached"), dest);
}

#[test]
fn concurrent_installs_leave_one_file() {
    let server = Server::start();
    let body = vec![7u8; 4 * 1024 * 1024];
    server.release(VERSION, ASSET, &body, &body);
    let cache = tempfile::tempdir().expect("tempdir");
    let dest = dest(&cache);
    let url = server.url(VERSION, ASSET);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| install(&url, &dest, &ChecksumSource::Sidecar)))
            .collect();
        for handle in handles {
            handle.join().expect("join").expect("installs");
        }
    });
    assert_eq!(std::fs::read(&dest).expect("read"), body);
    let dir = dest.parent().expect("dir");
    assert!(leftovers(dir).is_empty());
    assert_eq!(std::fs::read_dir(dir).expect("dir").count(), 1);
}

#[cfg(unix)]
#[test]
fn read_only_cache_names_the_path() {
    use std::os::unix::fs::PermissionsExt;
    let server = Server::start();
    server.release(VERSION, ASSET, b"binary", b"binary");
    let cache = tempfile::tempdir().expect("tempdir");
    let dest = dest(&cache);
    let dir = dest.parent().expect("dir");
    std::fs::create_dir_all(dir).expect("mkdir");
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555)).expect("chmod");
    if std::fs::write(dir.join("probe"), b"").is_ok() {
        return;
    }
    let err = install(&server.url(VERSION, ASSET), &dest, &ChecksumSource::Sidecar)
        .expect_err("read-only");
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    assert!(
        err.starts_with(&format!(
            "dx: cannot write the dx cache at {}: ",
            dir.display()
        )),
        "{err}"
    );
}
