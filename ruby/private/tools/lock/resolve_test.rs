//! The ruby lock step writes the lock and `Gemfile.lock` from index fixtures with an empty `PATH` and no network.

use std::cell::RefCell;
use std::io::Write;

use dx_lock::{Args, Error, Fetch, Lock, Outcome, run};
use ruby_lock::{Download, Ruby};

const GEMFILE: &str = "source \"https://rubygems.org\"\n\ngem \"rainbow\", \"~> 3.0\"\n\ngroup :test do\n  gem \"rspec-core\"\nend\n";

const RAINBOW_INFO: &str = "---\n2.2.2 |checksum:00\n3.0.0 |checksum:OLDRAINBOW\n3.1.1 |checksum:RAINBOW\n4.0.0 |checksum:00\n";
const RSPEC_INFO: &str = "---\n3.13.7 rspec-support:~> 3.13.0|checksum:RSPEC,ruby:>= 2.5\n3.14.0.rc1 rspec-support:~> 3.14.0|checksum:00\n";
const SUPPORT_INFO: &str = "---\n3.13.8 |checksum:SUPPORT\n3.14.0 |checksum:00\n";

fn tar(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (path, bytes) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(0);
        builder
            .append_data(&mut header, path, *bytes)
            .expect("tar entry");
    }
    builder.into_inner().expect("tar")
}

fn gz(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).expect("gzip");
    encoder.finish().expect("gzip")
}

fn gem(require_path: &str, files: &[&str]) -> Vec<u8> {
    let metadata = format!(
        "--- !ruby/object:Gem::Specification\nname: x\nrequire_paths:\n- {require_path}\nfiles: []\n"
    );
    let entries: Vec<(&str, &[u8])> = files.iter().map(|file| (*file, b"".as_slice())).collect();
    let data = gz(&tar(&entries));
    let metadata = gz(metadata.as_bytes());
    tar(&[("metadata.gz", &metadata), ("data.tar.gz", &data)])
}

struct Index {
    gems: Vec<(String, Vec<u8>)>,
    seen: RefCell<Vec<String>>,
}

impl Index {
    fn new() -> Self {
        let gems = vec![
            ("rainbow-3.0.0".to_owned(), gem("lib", &["lib/rainbow.rb"])),
            (
                "rainbow-3.1.1".to_owned(),
                gem("lib", &["lib/rainbow.rb", "lib/rainbow/color.rb"]),
            ),
            (
                "rspec-core-3.13.7".to_owned(),
                gem("lib", &["lib/rspec/core.rb", "exe/rspec"]),
            ),
            (
                "rspec-support-3.13.8".to_owned(),
                gem("lib", &["lib/rspec/support.rb"]),
            ),
        ];
        Self {
            gems,
            seen: RefCell::default(),
        }
    }

    fn sha(&self, gem: &str) -> String {
        let (_, bytes) = self.gems.iter().find(|(name, _)| name == gem).expect("gem");
        dx_digest::sha256_hex(bytes)
    }
}

impl Download for Index {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.seen.borrow_mut().push(url.to_owned());
        let info = |text: &str| {
            Ok(text
                .replace("OLDRAINBOW", &self.sha("rainbow-3.0.0"))
                .replace("RAINBOW", &self.sha("rainbow-3.1.1"))
                .replace("RSPEC", &self.sha("rspec-core-3.13.7"))
                .replace("SUPPORT", &self.sha("rspec-support-3.13.8"))
                .into_bytes())
        };
        match url {
            "https://rubygems.org/info/rainbow" => info(RAINBOW_INFO),
            "https://rubygems.org/info/rspec-core" => info(RSPEC_INFO),
            "https://rubygems.org/info/rspec-support" => info(SUPPORT_INFO),
            other => {
                let stem = other
                    .strip_prefix("https://rubygems.org/gems/")
                    .and_then(|rest| rest.strip_suffix(".gem"));
                self.gems
                    .iter()
                    .find(|(name, _)| Some(name.as_str()) == stem)
                    .map(|(_, bytes)| bytes.clone())
                    .ok_or_else(|| Error::new(format!("unexpected download of {other}")))
            }
        }
    }
}

type Summary = (String, String, Vec<String>, Vec<String>, bool);

struct NoFetch;

impl Fetch for NoFetch {
    fn sha256(&self, url: &str) -> Result<String, Error> {
        Err(Error::new(format!("unexpected download of {url}")))
    }
}

fn set(gemfile: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("set directory");
    std::fs::write(dir.path().join("Gemfile"), gemfile).expect("Gemfile");
    dir
}

fn lock(dir: &std::path::Path, index: &Index, check: bool) -> Result<Outcome, Error> {
    let hub = dx_families::hub("ruby").expect("ruby is registered");
    let spec = Ruby::spec(hub);
    run(
        &spec,
        &Args {
            dir: dir.to_owned(),
            check,
            upgrade: Vec::new(),
        },
        &Ruby::new(&spec, index),
        &NoFetch,
    )
}

#[test]
fn index_fixtures_write_the_lock_and_gemfile_lock() {
    let dir = set(GEMFILE);
    let index = Index::new();
    assert_eq!(lock(dir.path(), &index, false), Ok(Outcome::Written));
    let text = std::fs::read_to_string(dir.path().join("ruby.lock.json")).expect("lock");
    let written = Lock::from_json(&text).expect("lock parses");
    let summary: Vec<Summary> = written
        .packages
        .iter()
        .map(|package| {
            (
                package.name.clone(),
                package.version.clone(),
                package.deps.clone(),
                package.provides.clone(),
                package.dev,
            )
        })
        .collect();
    let strings = |items: &[&str]| {
        items
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        summary,
        vec![
            (
                "rainbow".to_owned(),
                "3.1.1".to_owned(),
                strings(&[]),
                strings(&["rainbow"]),
                false
            ),
            (
                "rspec-core".to_owned(),
                "3.13.7".to_owned(),
                strings(&["rspec-support"]),
                strings(&["rspec/core"]),
                true
            ),
            (
                "rspec-support".to_owned(),
                "3.13.8".to_owned(),
                strings(&[]),
                strings(&["rspec/support"]),
                true
            ),
        ]
    );
    let archive = &written.packages.first().expect("rainbow").archives;
    assert_eq!(
        archive.first().map(|a| a.urls.clone()),
        Some(vec![
            "https://rubygems.org/gems/rainbow-3.1.1.gem".to_owned()
        ])
    );
    assert_eq!(
        archive.first().map(|a| a.sha256.clone()),
        Some(index.sha("rainbow-3.1.1"))
    );
    let state = std::fs::read_to_string(dir.path().join("Gemfile.lock")).expect("state");
    assert_eq!(
        state,
        format!(
            "GEM\n  remote: https://rubygems.org/\n  specs:\n    rainbow (3.1.1)\n    rspec-core (3.13.7)\n      rspec-support (~> 3.13.0)\n    rspec-support (3.13.8)\n\nPLATFORMS\n  ruby\n\nDEPENDENCIES\n  rainbow (~> 3.0)\n  rspec-core\n\nCHECKSUMS\n  rainbow (3.1.1) sha256={}\n  rspec-core (3.13.7) sha256={}\n  rspec-support (3.13.8) sha256={}\n",
            index.sha("rainbow-3.1.1"),
            index.sha("rspec-core-3.13.7"),
            index.sha("rspec-support-3.13.8"),
        )
    );
    assert_eq!(lock(dir.path(), &Index::new(), true), Ok(Outcome::Fresh));
}

#[test]
fn locked_versions_are_kept_and_unchanged_gems_are_not_downloaded() {
    let dir = set(GEMFILE);
    assert_eq!(lock(dir.path(), &Index::new(), false), Ok(Outcome::Written));
    let state = dir.path().join("Gemfile.lock");
    let pinned = std::fs::read_to_string(&state)
        .expect("state")
        .replace("rainbow (3.1.1)\n", "rainbow (3.0.0)\n")
        + "\nBUNDLED WITH\n   2.6.9\n";
    std::fs::write(&state, pinned).expect("state");
    std::fs::write(dir.path().join("Gemfile"), format!("{GEMFILE}\n")).expect("Gemfile");
    let index = Index::new();
    assert_eq!(lock(dir.path(), &index, false), Ok(Outcome::Written));
    assert_eq!(
        index
            .seen
            .borrow()
            .iter()
            .filter(|url| url.ends_with(".gem"))
            .cloned()
            .collect::<Vec<_>>(),
        vec!["https://rubygems.org/gems/rainbow-3.0.0.gem".to_owned()]
    );
    let written = std::fs::read_to_string(&state).expect("state");
    assert!(written.contains("    rainbow (3.0.0)\n"), "{written}");
    assert!(written.ends_with("\nBUNDLED WITH\n   2.6.9\n"), "{written}");
}

#[test]
fn a_git_gem_fails() {
    let dir = set("source \"https://rubygems.org\"\ngem \"x\", git: \"https://example.com/x\"\n");
    let error = lock(dir.path(), &Index::new(), false).expect_err("refused");
    assert!(error.to_string().contains("uses git:"), "{error}");
}

#[test]
fn an_unsatisfiable_requirement_fails() {
    let dir = set("source \"https://rubygems.org\"\ngem \"rainbow\", \">= 9\"\n");
    let error = lock(dir.path(), &Index::new(), false).expect_err("refused");
    assert!(
        error
            .to_string()
            .contains("no version of rainbow satisfies >= 9"),
        "{error}"
    );
}
