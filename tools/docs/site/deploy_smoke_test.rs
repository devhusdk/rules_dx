//! Smoke-tests one served docs site against the staged artifact bytes.

use std::path::Path;
use std::thread;
use std::time::Duration;

use dx_site_browser::{wait_for, Browser};

mod preview_server;
use preview_server::{firefox_binary, search_query, user_site, Server};

const DEPLOYED_URL: &str = "DX_PAGES_URL";
const READINESS_ATTEMPTS: u32 = 10;
const READINESS_PAUSE: Duration = Duration::from_secs(3);
const PROBE_TIMEOUT: u64 = 15;
const DESKTOP: (u64, u64) = (1280, 900);
const IDENTITY_ROUTES: [&str; 2] = ["index.html", "searchindex.js"];
const REPRESENTATIVE_ROUTES: [&str; 4] = [
    "docs/cli/commands/docs.html",
    "docs/github-ci.html",
    "examples/adopt-cpp/index.html",
    "404.html",
];

struct Fetched {
    status: u32,
    content_type: String,
    body: String,
}

fn fetch(browser: &mut Browser, url: &str) -> Fetched {
    let encoded = dx_testing::serde_json::to_string(url).expect("encode one smoke URL");
    let source = format!(
        "window.__dxSmoke = null; \
         fetch({encoded}).then(function (response) {{ \
           return response.text().then(function (body) {{ \
             window.__dxSmoke = {{ status: response.status, \
               contentType: response.headers.get('content-type') || '', body: body }}; \
           }}); \
         }}).catch(function () {{ \
           window.__dxSmoke = {{ status: 0, contentType: '', body: '' }}; \
         }}); \
         return 1;"
    );
    browser
        .script(&source)
        .unwrap_or_else(|error| panic!("probe {url}: {error}"));
    let settled = wait_for(
        browser,
        "return typeof window.__dxSmoke === 'object' && window.__dxSmoke !== null",
        PROBE_TIMEOUT,
    )
    .unwrap_or_else(|error| panic!("poll {url}: {error}"));
    if !settled {
        return Fetched {
            status: 0,
            content_type: String::new(),
            body: String::new(),
        };
    }
    let status = browser
        .text("return String(window.__dxSmoke.status)")
        .unwrap_or_default()
        .parse()
        .unwrap_or(0);
    let content_type = browser
        .text("return window.__dxSmoke.contentType")
        .unwrap_or_default();
    let body = browser
        .text("return window.__dxSmoke.body")
        .unwrap_or_default();
    Fetched {
        status,
        content_type,
        body,
    }
}

fn waits_for(browser: &mut Browser, base: &str) -> bool {
    for attempt in 0..READINESS_ATTEMPTS {
        let probe = format!("{base}index.html");
        if browser.navigate(base).is_ok() && fetch(browser, &probe).status == 200 {
            return true;
        }
        if attempt + 1 < READINESS_ATTEMPTS {
            thread::sleep(READINESS_PAUSE);
        }
    }
    false
}

fn smoke(browser: &mut Browser, base: &str, staged: &Path) {
    assert!(
        waits_for(browser, base),
        "the site at {base} answered no HTTP 200 within {READINESS_ATTEMPTS} attempts"
    );

    for route in IDENTITY_ROUTES {
        let url = format!("{base}{route}");
        let served = fetch(browser, &url);
        assert_eq!(served.status, 200, "{url} served status {}", served.status);
        let expected = std::fs::read_to_string(staged.join(route))
            .unwrap_or_else(|error| panic!("read the staged {route}: {error}"));
        assert_eq!(
            served.body, expected,
            "{url} differs from the staged artifact"
        );
    }

    let css_url = format!("{base}css/general.css");
    let css = fetch(browser, &css_url);
    assert_eq!(css.status, 200, "{css_url} served status {}", css.status);
    assert!(
        css.content_type.starts_with("text/css"),
        "{css_url} served content type {:?}",
        css.content_type
    );

    for route in REPRESENTATIVE_ROUTES {
        let url = format!("{base}{route}");
        let served = fetch(browser, &url);
        assert_eq!(served.status, 200, "{url} served status {}", served.status);
        assert!(!served.body.is_empty(), "{url} served an empty document");
        assert!(
            served.content_type.starts_with("text/html"),
            "{url} served content type {:?}",
            served.content_type
        );
    }

    browser
        .navigate(base)
        .unwrap_or_else(|error| panic!("load the landing page at {base}: {error}"));
    assert_eq!(
        browser.text("return document.title").expect("a title"),
        "rules_dx - rules_dx"
    );
    assert!(
        browser
            .count("return document.querySelectorAll('#sidebar a').length")
            .expect("sidebar links")
            > 0,
        "the sidebar at {base} lists no page"
    );
    let stylesheet = browser
        .truthy(
            "return Array.from(document.styleSheets).some(function (sheet) {\
               try { return Array.from(sheet.cssRules).length > 0; } catch (error) { return false; }\
             });",
        )
        .expect("the loaded stylesheet");
    assert!(stylesheet, "no stylesheet rule applies at {base}");

    search_query(browser, "scope");
    let hit = browser
        .text("return document.querySelector('#searchresults a').href")
        .expect("the first search hit");
    browser
        .navigate(&hit)
        .unwrap_or_else(|error| panic!("follow the search hit {hit}: {error}"));
    let title = browser
        .text("return document.title")
        .expect("the search hit title");
    assert!(!title.is_empty(), "the search hit {hit} rendered no title");
}

#[test]
fn the_served_site_matches_the_staged_artifact_and_searches() {
    let staged = user_site();
    let mut browser = {
        let profile = dx_testing::mkscratch("firefox-profile").expect("a browser profile");
        Browser::start(&firefox_binary(), &profile)
            .unwrap_or_else(|error| panic!("start Firefox: {error}"))
    };
    browser
        .set_viewport(DESKTOP.0, DESKTOP.1)
        .expect("size the viewport");

    let server = Server::start(staged.clone(), "rules_dx");
    let local = server.url("/rules_dx/");
    smoke(&mut browser, &local, &staged);

    let deployed = match std::env::var(DEPLOYED_URL) {
        Ok(value) => value,
        Err(_) => return,
    };
    assert!(
        !deployed.is_empty(),
        "{DEPLOYED_URL} is set but names no deployment URL"
    );
    let base = if deployed.ends_with('/') {
        deployed
    } else {
        format!("{deployed}/")
    };
    smoke(&mut browser, &base, &staged);
}
