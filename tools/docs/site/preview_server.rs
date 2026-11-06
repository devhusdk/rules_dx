//! Serves a rendered docs site over loopback for the Firefox-driven site tests.

use std::path::PathBuf;

use dx_docs_serve::{Handle, Site};
use dx_site_browser::{Browser, wait_for};

const FIREFOX: &str = "DX_FIREFOX";
const USER_SITE: &str = "DX_USER_SITE";

pub struct Server {
    port: u16,
    _site: Handle,
}

impl Server {
    pub fn start(root: PathBuf, base: &str) -> Self {
        let site = Site::bind("127.0.0.1", 0, root, base).expect("bind a loopback port");
        let port = site.local_addr().expect("local addr").port();
        Server {
            port,
            _site: site.spawn().expect("spawn"),
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }
}

pub fn firefox_binary() -> PathBuf {
    let rel = std::env::var(FIREFOX).expect("DX_FIREFOX must name the acquired browser binary");
    let path = dx_testing::resolve_runfiles(&rel);
    assert!(path.is_file(), "not a file: {}", path.display());
    path
}

pub fn user_site() -> PathBuf {
    let rel = std::env::var(USER_SITE).expect("DX_USER_SITE must name the rendered site");
    let path = dx_testing::resolve_runfiles(&rel);
    assert!(path.is_dir(), "not a directory: {}", path.display());
    path
}

pub fn search_query(browser: &mut Browser, term: &str) {
    browser.press("s").expect("focus the search bar");
    for character in term.chars() {
        browser
            .press(&character.to_string())
            .unwrap_or_else(|error| panic!("type {character:?}: {error}"));
    }
    assert!(
        wait_for(
            browser,
            "return document.querySelectorAll('#searchresults li').length > 0",
            30
        )
        .unwrap_or_else(|error| panic!("poll the search results: {error}")),
        "searching for {term:?} returned no result"
    );
}
