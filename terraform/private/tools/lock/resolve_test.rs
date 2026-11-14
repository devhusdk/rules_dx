//! The terraform lock step writes the golden lock and state from the registry fixture with an empty `PATH` and no network.

use std::cell::RefCell;

use dx_lock::{Args, Error, Fetch, Outcome, run};
use terraform_lock::{Registry, Terraform, requirements, satisfies};

const MAIN: &str = "terraform {
  # required_providers { ignored = \"1\" }
  required_providers {
    null = {
      source  = \"hashicorp/null\"
      version = \"~> 3.2\"
    }
  }
}
";

const SECOND: &str = "terraform {
  required_providers {
    null = {
      source  = \"registry.terraform.io/hashicorp/null\"
      version = \">= 3.2.3, != 3.2.3\"
    }
  }
}
";

struct Site(RefCell<Vec<String>>);

impl Registry for Site {
    fn get(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.borrow_mut().push(url.to_owned());
        let var = if url.ends_with("/versions") {
            "VERSIONS"
        } else if url.ends_with("/download/linux/amd64") {
            "DOWNLOAD_LINUX_AMD64"
        } else if url.ends_with("/download/darwin/arm64") {
            "DOWNLOAD_DARWIN_ARM64"
        } else {
            return Err(Error::new(format!("unexpected request {url}")));
        };
        std::fs::read(dx_testing::env_file(var)).map_err(|error| Error::new(error.to_string()))
    }
}

struct NoFetch;

impl Fetch for NoFetch {
    fn sha256(&self, url: &str) -> Result<String, Error> {
        Err(Error::new(format!("unexpected download of {url}")))
    }
}

fn lock(dir: &std::path::Path, site: &Site) -> Result<Outcome, Error> {
    let hub = dx_families::hub("terraform").expect("terraform is registered");
    run(
        &Terraform::spec(hub),
        &Args {
            dir: dir.to_owned(),
            check: false,
            upgrade: Vec::new(),
        },
        &Terraform::new(site, hub),
        &NoFetch,
    )
}

#[test]
fn the_registry_fixture_writes_the_golden_lock_for_two_platforms() {
    let dir = tempfile::tempdir().expect("set directory");
    std::fs::write(dir.path().join("main.tf"), MAIN).expect("main.tf");
    std::fs::write(dir.path().join("second.tf"), SECOND).expect("second.tf");
    let site = Site(RefCell::default());
    assert_eq!(lock(dir.path(), &site), Ok(Outcome::Written));
    assert_eq!(
        *site.0.borrow(),
        [
            "https://registry.terraform.io/v1/providers/hashicorp/null/versions",
            "https://registry.terraform.io/v1/providers/hashicorp/null/3.2.4/download/darwin/arm64",
            "https://registry.terraform.io/v1/providers/hashicorp/null/3.2.4/download/linux/amd64",
        ]
    );
    dx_testing::snapshot_diff(
        &dx_testing::env_file("GOLDEN"),
        &dir.path().join("terraform.lock.json"),
        Some("terraform/private/tools/lock/testdata/golden.lock.json"),
    )
    .expect("the lock is the golden lock");
    dx_testing::snapshot_diff(
        &dx_testing::env_file("GOLDEN_STATE"),
        &dir.path().join(".terraform.lock.hcl"),
        Some("terraform/private/tools/lock/testdata/golden.terraform.lock.hcl"),
    )
    .expect("the state is the golden state");
    site.0.borrow_mut().clear();
    std::fs::write(dir.path().join("second.tf"), "").expect("second.tf");
    assert_eq!(lock(dir.path(), &site), Ok(Outcome::Written));
    assert!(site.0.borrow().is_empty());
}

#[test]
fn a_provider_without_a_release_fails() {
    let dir = tempfile::tempdir().expect("set directory");
    std::fs::write(
        dir.path().join("a.tf"),
        "terraform { required_providers { null = { version = \"> 9\" } } }\n",
    )
    .expect("a");
    let error = lock(dir.path(), &Site(RefCell::default())).expect_err("no release");
    assert!(error.to_string().contains("no release satisfies"), "{error}");
}

#[test]
fn sources_off_the_public_registry_fail() {
    assert!(requirements("terraform { required_providers { x = { source = \"example.com/a/b\" } } }").is_err());
    let found = requirements("terraform { required_providers { aws = \"~> 5.0\" } }").expect("legacy form");
    assert_eq!(found.len(), 1);
}

#[test]
fn constraints_follow_terraform() {
    for (version, constraint, holds) in [
        ("3.2.4", "~> 3.2", true),
        ("4.0.0", "~> 3.2", false),
        ("3.2.9", "~> 3.2.1", true),
        ("3.3.0", "~> 3.2.1", false),
        ("1.5.0", ">= 1.2, < 1.5", false),
        ("1.4.9", ">= 1.2, < 1.5", true),
        ("1.0.0", "!= 1.0.0", false),
        ("2.0.0", "", true),
    ] {
        assert_eq!(satisfies(version, constraint), Ok(holds), "{version} {constraint}");
    }
}
