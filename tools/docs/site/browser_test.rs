//! Serves the rendered site and drives it in a real Firefox at every hosting base.

use std::thread;
use std::time::Duration;

use dx_site_browser::{wait_for, Browser};

mod preview_server;
use preview_server::{firefox_binary, search_query, user_site, Server};

const DESKTOP: (u64, u64) = (1280, 900);
const PHONE: (u64, u64) = (390, 780);
const WIDE: (u64, u64) = (1600, 1000);

/// A browser that owns its scratch profile; fields drop in order, so Firefox
/// stops before the profile dir is removed.
struct ProfiledBrowser {
    browser: Browser,
    _profile: dx_test_scratch::TempDir,
}

impl std::ops::Deref for ProfiledBrowser {
    type Target = Browser;
    fn deref(&self) -> &Browser {
        &self.browser
    }
}

impl std::ops::DerefMut for ProfiledBrowser {
    fn deref_mut(&mut self) -> &mut Browser {
        &mut self.browser
    }
}

fn browser_at(base: &str) -> ProfiledBrowser {
    let profile = dx_test_scratch::scratch("firefox-profile");
    let browser = Browser::start(&firefox_binary(), profile.path())
        .unwrap_or_else(|error| panic!("start Firefox for the {base} base: {error}"));
    ProfiledBrowser {
        browser,
        _profile: profile,
    }
}

#[test]
fn the_site_navigates_searches_and_reaches_every_command_page_in_a_browser() {
    let server = Server::start(user_site(), "");
    let mut browser = browser_at("root");
    browser
        .set_viewport(DESKTOP.0, DESKTOP.1)
        .expect("size the viewport");
    browser
        .navigate(&server.url("/"))
        .expect("load the landing page");

    assert_eq!(
        browser.text("return document.title").expect("a title"),
        "rules_dx - rules_dx"
    );
    assert_eq!(
        browser
            .text("return document.querySelector('main h1, #content h1').textContent.trim()")
            .expect("the landing heading"),
        "rules_dx"
    );
    assert!(
        browser
            .count("return document.querySelectorAll('#sidebar a').length")
            .expect("sidebar links")
            >= 30,
        "the sidebar must list the whole book"
    );

    let links = browser
        .script(
            "return Array.from(document.querySelectorAll('#sidebar a'))\
             .map(function (a) { return a.getAttribute('href'); })",
        )
        .expect("sidebar routes");
    let routes: Vec<String> = links
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .filter(|href| href.ends_with(".html"))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    assert!(
        routes.len() >= 30,
        "the sidebar names {} rendered pages",
        routes.len()
    );
    let mut crates = std::collections::BTreeSet::new();
    let routes: Vec<String> = routes
        .into_iter()
        .filter(|route| match route.strip_prefix("api/") {
            Some(rest) => {
                crates.insert(rest.rsplit_once('/').map_or("", |(dir, _)| dir).to_string())
            }
            None => true,
        })
        .collect();

    for route in &routes {
        browser
            .navigate(&server.url(&format!("/{route}")))
            .expect("load every route");
        let title = browser.text("return document.title").expect("a page title");
        assert!(!title.is_empty(), "{route} rendered no title");
        let broken = browser.count("return document.querySelectorAll('img[src=\"\"]').length");
        assert_eq!(
            broken.unwrap_or_default(),
            0,
            "{route} has an empty image source"
        );
    }

    browser
        .navigate(&server.url("/"))
        .expect("return to the landing page");
    search_query(&mut browser, "scope");
    assert!(
        browser
            .text("return document.querySelector('#searchresults-header').textContent")
            .expect("the search summary")
            .contains("scope"),
        "the search summary must name the query"
    );
    let first = browser
        .text("return document.querySelector('#searchresults a').getAttribute('href')")
        .expect("the first result");
    let route = first.split(['?', '#']).next().unwrap_or_default();
    assert!(
        route.ends_with(".html"),
        "a search hit must link a page: {first}"
    );
    browser
        .navigate(&server.url(&format!("/{first}")))
        .expect("follow the search hit");
    assert!(
        browser
            .truthy("return document.querySelector('h1') !== null")
            .expect("a heading"),
        "a search hit must render a page"
    );
}

#[test]
fn keyboard_navigation_walks_the_book_forward_and_back() {
    let server = Server::start(user_site(), "");
    let mut browser = browser_at("root");
    browser
        .set_viewport(DESKTOP.0, DESKTOP.1)
        .expect("size the viewport");
    browser
        .navigate(&server.url("/index.html"))
        .expect("load the landing page");

    let landing = browser
        .text("return location.pathname")
        .expect("the landing path");
    assert_eq!(landing, "/index.html");
    browser.press("\u{e014}").expect("press the right arrow");
    let forward = browser
        .text("return location.pathname")
        .expect("the next path");
    assert_ne!(forward, landing, "the right arrow must open the next page");
    browser.press("\u{e012}").expect("press the left arrow");
    assert_eq!(
        browser
            .text("return location.pathname")
            .expect("the back path"),
        landing,
        "the left arrow must return to the landing page"
    );
    browser.press("/").expect("press the slash key");
    assert_eq!(
        browser
            .text("return document.activeElement.id")
            .expect("the focused element"),
        "searchbar",
        "the slash key must focus the search bar"
    );
}

#[test]
fn the_sidebar_toggle_hides_the_navigation_and_focus_is_visible() {
    let server = Server::start(user_site(), "");
    let mut browser = browser_at("root");
    browser
        .set_viewport(DESKTOP.0, DESKTOP.1)
        .expect("size the viewport");
    browser
        .navigate(&server.url("/"))
        .expect("load the landing page");

    let shown = browser
        .text("return getComputedStyle(document.getElementById('sidebar')).display")
        .expect("the sidebar display");
    assert_ne!(shown, "none", "the sidebar must start open");
    assert!(
        browser
            .truthy("return document.getElementById('sidebar').getBoundingClientRect().right > 0")
            .expect("the open sidebar box"),
        "the sidebar must start on screen"
    );
    browser
        .script("document.getElementById('sidebar-toggle').click(); return 1")
        .expect("toggle the sidebar");
    assert!(
        wait_for(
            &mut browser,
            "return document.getElementById('sidebar').getAttribute('aria-hidden') === 'true'",
            30,
        )
        .expect("poll the hidden sidebar"),
        "the toggle must mark the navigation hidden"
    );
    thread::sleep(Duration::from_millis(800));
    assert!(
        browser
            .truthy("return document.getElementById('sidebar').getBoundingClientRect().right <= 0")
            .expect("the hidden sidebar box"),
        "the toggle must move the navigation off screen"
    );

    let contrast = browser
        .text("return getComputedStyle(document.body).getPropertyValue('--fg').trim()")
        .expect("the body foreground");
    assert!(
        !contrast.is_empty(),
        "the stylesheet must define the body foreground"
    );
    assert!(
        browser
            .truthy(
                "return Array.from(document.styleSheets).some(function (sheet) {\
                   try { return Array.from(sheet.cssRules).some(function (rule) {\
                     return rule.cssText.indexOf(':focus') >= 0; }); } catch (error) { return false; }\
                 });",
            )
            .expect("the focus rule"),
        "the stylesheet must style keyboard focus"
    );
}

#[test]
fn the_same_build_works_from_the_project_subdirectory_base() {
    let server = Server::start(user_site(), "rules_dx");
    let mut browser = browser_at("subdirectory");
    browser
        .set_viewport(DESKTOP.0, DESKTOP.1)
        .expect("size the viewport");
    browser
        .navigate(&server.url("/rules_dx/"))
        .expect("load the landing page");

    assert_eq!(
        browser.text("return document.title").expect("a title"),
        "rules_dx - rules_dx"
    );
    let stylesheet = browser
        .truthy(
            "return Array.from(document.styleSheets).some(function (sheet) {\
               try { return Array.from(sheet.cssRules).length > 0; } catch (error) { return false; }\
             });",
        )
        .expect("the loaded stylesheet");
    assert!(stylesheet, "the stylesheet must load from /rules_dx/");

    browser
        .navigate(&server.url("/rules_dx/docs/cli/commands/docs.html"))
        .expect("load a nested page from the subdirectory base");
    assert_eq!(
        browser
            .text("return document.title")
            .expect("a nested title"),
        "Docs - rules_dx"
    );
    assert!(
        browser
            .truthy("return getComputedStyle(document.querySelector('pre code')).color !== ''")
            .expect("a highlighted code block"),
        "the nested page must render highlighted code"
    );
    search_query(&mut browser, "build");
    let first = browser
        .text("return document.querySelector('#searchresults a').getAttribute('href')")
        .expect("the first result");
    assert!(
        first.contains("../../"),
        "a search hit on a nested page must stay inside the base: {first}"
    );
}

#[test]
fn a_phone_sized_viewport_collapses_the_navigation() {
    let server = Server::start(user_site(), "rules_dx");
    let mut browser = browser_at("subdirectory");
    browser
        .set_viewport(PHONE.0, PHONE.1)
        .expect("size the viewport");
    browser
        .navigate(&server.url("/rules_dx/"))
        .expect("load the landing page");

    let width = browser
        .text("return String(window.innerWidth)")
        .expect("the viewport width");
    assert!(
        width.parse::<u64>().is_ok_and(|value| value <= 500),
        "the phone viewport reports {width} CSS pixels"
    );
    assert_ne!(
        browser
            .text("return getComputedStyle(document.getElementById('sidebar-toggle')).display")
            .expect("the toggle display"),
        "none",
        "a phone must still be able to open the navigation"
    );
    browser
        .script("document.getElementById('sidebar-toggle').click(); return 1")
        .expect("open the navigation on a phone");
    thread::sleep(Duration::from_millis(800));
    assert_ne!(
        browser
            .text("return getComputedStyle(document.getElementById('sidebar')).display")
            .expect("the phone sidebar display"),
        "none",
        "the toggle must open the navigation on a phone"
    );
    let overflow = browser
        .count("return document.documentElement.scrollWidth > window.innerWidth + 2 ? 1 : 0");
    assert_eq!(
        overflow.unwrap_or_default(),
        0,
        "the phone layout must not scroll sideways"
    );
}

#[test]
fn a_wide_viewport_shows_the_navigation_and_the_search_bar() {
    let server = Server::start(user_site(), "rules_dx");
    let mut browser = browser_at("subdirectory");
    browser
        .set_viewport(WIDE.0, WIDE.1)
        .expect("size the viewport");
    browser
        .navigate(&server.url("/rules_dx/"))
        .expect("load the landing page");

    assert_ne!(
        browser
            .text("return getComputedStyle(document.getElementById('sidebar')).display")
            .expect("the wide sidebar display"),
        "none",
        "a wide viewport must show the navigation"
    );
    assert_ne!(
        browser
            .text("return getComputedStyle(document.getElementById('searchbar')).display")
            .expect("the search bar display"),
        "none",
        "a wide viewport must show the search bar"
    );
    let overflow = browser
        .count("return document.documentElement.scrollWidth > window.innerWidth + 2 ? 1 : 0");
    assert_eq!(
        overflow.unwrap_or_default(),
        0,
        "a wide layout must not scroll sideways"
    );
}

#[test]
fn the_browser_reports_a_page_it_cannot_load() {
    let server = Server::start(user_site(), "");
    let mut browser = browser_at("root");
    browser
        .navigate(&server.url("/no-such-page.html"))
        .expect("request a missing page");
    let text = browser
        .text("return document.body.textContent")
        .expect("the 404 body");
    assert!(
        text.contains("Not Found"),
        "an unknown route must render the server's refusal: {text}"
    );
}
