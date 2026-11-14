//! The nuget lock step writes the golden lock from registry fixtures with an empty `PATH` and no network.

use std::cell::RefCell;

use dx_lock::{Args, Error, Fetch, Outcome, run};
use nuget_lock::{Download, Nuget, Tool, lock_groups, manifest_frameworks, normalize_version, tools};

const ALPHA: &str =
    "https://api.nuget.org/v3-flatcontainer/alpha.core/1.2.0/alpha.core.1.2.0.nupkg";
const BETA: &str = "https://api.nuget.org/v3-flatcontainer/beta/2.0.0/beta.2.0.0.nupkg";

const MANIFEST: &str = "\
source https://api.nuget.org/v3/index.json
framework: net10.0
nuget Beta

group tools
source https://api.nuget.org/v3/index.json
framework: net10.0
nuget Alpha.Core
";

const STATE: &str = "\
RESTRICTION: == net10.0
NUGET
  remote: https://api.nuget.org/v3/index.json
    Alpha.Core (1.2)
    Beta (2.0.0)
      Alpha.Core (>= 1.0)
      Gamma (>= 1.0) - restriction: < net20

GROUP tools
RESTRICTION: == net10.0
NUGET
  remote: https://api.nuget.org/v3/index.json
    Alpha.Core (1.2.0)
";

struct Registry(RefCell<Vec<String>>);

impl Download for Registry {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.borrow_mut().push(url.to_owned());
        let fixture = match url {
            ALPHA => "ALPHA",
            BETA => "BETA",
            other => return Err(Error::new(format!("unexpected download of {other}"))),
        };
        std::fs::read(dx_testing::env_file(fixture)).map_err(|error| Error::new(error.to_string()))
    }
}

struct NoFetch;

impl Fetch for NoFetch {
    fn sha256(&self, url: &str) -> Result<String, Error> {
        Err(Error::new(format!("unexpected download of {url}")))
    }
}

fn set(manifest: &str, state: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("set directory");
    std::fs::write(dir.path().join("paket.dependencies"), manifest).expect("manifest");
    std::fs::write(dir.path().join("paket.lock"), state).expect("state");
    dir
}

fn lock(dir: &std::path::Path, registry: &Registry) -> Result<Outcome, Error> {
    let hub = dx_families::hub("nuget").expect("nuget is registered");
    let spec = Nuget::spec(hub);
    run(
        &spec,
        &Args {
            dir: dir.to_owned(),
            check: false,
            upgrade: Vec::new(),
        },
        &Nuget::new(&spec, registry),
        &NoFetch,
    )
}

#[test]
fn registry_fixtures_write_the_golden_lock() {
    let dir = set(MANIFEST, STATE);
    assert_eq!(
        lock(dir.path(), &Registry(RefCell::default())),
        Ok(Outcome::Written)
    );
    dx_testing::snapshot_diff(
        &dx_testing::env_file("GOLDEN"),
        &dir.path().join("nuget.lock.json"),
        Some("csharp/private/tools/lock/testdata/golden.lock.json"),
    )
    .expect("the lock matches the golden");
    let state = std::fs::read_to_string(dir.path().join("paket.lock")).expect("state");
    assert_eq!(state, STATE);
}

#[test]
fn unchanged_packages_are_kept_without_a_download() {
    let dir = set(MANIFEST, STATE);
    assert_eq!(
        lock(dir.path(), &Registry(RefCell::default())),
        Ok(Outcome::Written)
    );
    std::fs::write(
        dir.path().join("paket.dependencies"),
        format!("{MANIFEST}\n"),
    )
    .expect("manifest");
    let registry = Registry(RefCell::default());
    assert_eq!(lock(dir.path(), &registry), Ok(Outcome::Written));
    assert_eq!(registry.0.borrow().as_slice(), Vec::<String>::new());
}

#[test]
fn a_group_missing_from_the_state_fails() {
    let dir = set(&format!("{MANIFEST}\ngroup docs\nnuget Beta\n"), STATE);
    let error = lock(dir.path(), &Registry(RefCell::default())).expect_err("refused");
    assert!(
        error.to_string().contains("paket.lock has no group docs"),
        "{error}"
    );
}

#[test]
fn another_source_fails() {
    let dir = set(
        "source https://example.com/v3/index.json\nnuget Beta\n",
        STATE,
    );
    let error = lock(dir.path(), &Registry(RefCell::default())).expect_err("refused");
    assert!(
        error
            .to_string()
            .contains("is not https://api.nuget.org/v3/index.json"),
        "{error}"
    );
}

#[test]
fn a_github_section_fails() {
    let error = lock_groups("GITHUB\n  remote: owner/repo\n").expect_err("refused");
    assert!(
        error
            .to_string()
            .contains("a GITHUB section has no NuGet archive"),
        "{error}"
    );
}

#[test]
fn versions_take_the_nuget_normal_form() {
    assert_eq!(normalize_version("1.62"), "1.62.0");
    assert_eq!(normalize_version("4.6.1"), "4.6.1");
    assert_eq!(normalize_version("1.2.3.0"), "1.2.3");
    assert_eq!(normalize_version("1.2.3.4"), "1.2.3.4");
    assert_eq!(normalize_version("01.002.0-beta.1+sha"), "1.2.0-beta.1");
}

#[test]
fn each_group_takes_its_framework_restriction() {
    let frameworks = manifest_frameworks(MANIFEST);
    assert_eq!(
        frameworks.get("main").map(Vec::as_slice),
        Some(["net10.0".to_owned()].as_slice())
    );
    assert_eq!(
        frameworks.get("tools").map(Vec::as_slice),
        Some(["net10.0".to_owned()].as_slice())
    );
}

#[test]
fn a_group_without_a_framework_fails() {
    let dir = set(
        "source https://api.nuget.org/v3/index.json\nnuget Beta\n",
        STATE,
    );
    let error = lock(dir.path(), &Registry(RefCell::default())).expect_err("refused");
    assert!(
        error
            .to_string()
            .contains("group main has no framework restriction"),
        "{error}"
    );
}

fn archive(files: &[(&str, &str)]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, text) in files {
        writer
            .start_file(
                *name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .expect("entry");
        std::io::Write::write_all(&mut writer, text.as_bytes()).expect("write");
    }
    writer.finish().expect("archive").into_inner()
}

#[test]
fn tool_packages_name_their_commands_per_framework() {
    let bytes = archive(&[
        (
            "Tool.nuspec",
            "<package><metadata><packageTypes><packageType name=\"DotnetTool\" /></packageTypes></metadata></package>",
        ),
        (
            "tools/net10.0/any/DotnetToolSettings.xml",
            "<DotNetCliTool Version=\"1\"><Commands><Command Name=\"tool\" EntryPoint=\"Tool.dll\" Runner=\"dotnet\" /></Commands></DotNetCliTool>",
        ),
    ]);
    let found = tools(&bytes).expect("tools");
    assert_eq!(
        found.get("net10.0").map(Vec::as_slice),
        Some(
            [Tool {
                name: "tool".to_owned(),
                entrypoint: "Tool.dll".to_owned(),
                runner: "dotnet".to_owned(),
            }]
            .as_slice()
        )
    );
}

#[test]
fn library_packages_have_no_commands() {
    let bytes = archive(&[
        ("Lib.nuspec", "<package><metadata /></package>"),
        ("tools/net10.0/any/DotnetToolSettings.xml", "<Commands />"),
    ]);
    assert!(tools(&bytes).expect("tools").is_empty());
}
