//! Checks the Ruby fixture shards and the YARD driver's and converter's failure handling.

use std::path::{Path, PathBuf};
use std::process::Command;

use documentation_ir::proto::{DocIr, Symbol, SymbolKind};
use documentation_ir::{decode_shard, validate_shard};

const PREFIX: &str = "ruby:geometry:";
const GEOMETRY: &str = "tools/docs/ruby/fixture/lib/geometry.rb";
const SHAPES: &str = "tools/docs/ruby/fixture/lib/geometry/shapes.rb";
const CRLF_SHAPES: &str = "tools/docs/ruby/fixture/crlf/shapes.rb";

fn runfile(var: &str) -> PathBuf {
    let rel = std::env::var(var).unwrap_or_else(|_| panic!("{var} must name a runfile"));
    dx_testing::resolve_runfiles(&rel)
}

fn shard(var: &str) -> DocIr {
    let bytes = std::fs::read(runfile(var)).expect("read the shard");
    decode_shard(&bytes).expect("the shard decodes")
}

fn find<'a>(shard: &'a DocIr, id: &str) -> Option<&'a Symbol> {
    shard.symbols.iter().find(|s| s.id == id)
}

fn symbol<'a>(shard: &'a DocIr, qualified: &str) -> &'a Symbol {
    let id = format!("{PREFIX}{qualified}");
    find(shard, &id).unwrap_or_else(|| panic!("{id} is missing"))
}

fn extension<'a>(symbol: &'a Symbol, key: &str) -> Option<&'a str> {
    symbol
        .extensions
        .iter()
        .find(|e| e.key == key)
        .map(|e| std::str::from_utf8(&e.value).expect("utf-8 extension"))
}

/// Returns the 1-based line of the first fixture line that contains `needle`.
fn line_of(var: &str, needle: &str) -> u64 {
    let text = std::fs::read_to_string(runfile(var)).expect("read the fixture");
    let index = text
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("{needle} is not in the fixture"));
    index as u64 + 1
}

#[test]
fn the_fixture_shard_maps_every_shape() {
    let shard = shard("DX_FIXTURE_SHARD");
    validate_shard(&shard).expect("validate_shard passes");
    assert_eq!(shard.language, "ruby");
    assert_eq!(shard.package, "geometry");
    for (qualified, kind, display) in [
        ("Geometry", SymbolKind::Module, "Geometry"),
        (
            "Geometry.DIMENSIONS",
            SymbolKind::Constant,
            "Geometry::DIMENSIONS",
        ),
        ("Geometry.Sized", SymbolKind::Module, "Geometry::Sized"),
        (
            "Geometry.Sized.zero~3F",
            SymbolKind::Method,
            "Geometry::Sized#zero?",
        ),
        ("Geometry.Shape", SymbolKind::Class, "Geometry::Shape"),
        (
            "Geometry.Shape.scale",
            SymbolKind::Method,
            "Geometry::Shape#scale",
        ),
        ("Geometry.Circle", SymbolKind::Class, "Geometry::Circle"),
        (
            "Geometry.Circle.area",
            SymbolKind::Method,
            "Geometry::Circle#area",
        ),
        (
            "Geometry.Circle.self.unit",
            SymbolKind::Method,
            "Geometry::Circle.unit",
        ),
        (
            "Geometry.Circle.empty~3F",
            SymbolKind::Method,
            "Geometry::Circle#empty?",
        ),
        (
            "Geometry.Circle.~3C~3D~3E",
            SymbolKind::Method,
            "Geometry::Circle#<=>",
        ),
        (
            "Geometry.Circle.radius",
            SymbolKind::Property,
            "Geometry::Circle#radius",
        ),
        (
            "Geometry.Circle.label",
            SymbolKind::Property,
            "Geometry::Circle#label",
        ),
        (
            "Geometry.Circle.initialize",
            SymbolKind::Method,
            "Geometry::Circle#initialize",
        ),
        (
            "Geometry.Circle.diameter",
            SymbolKind::Method,
            "Geometry::Circle#diameter",
        ),
        (
            "Geometry.Circle.shift",
            SymbolKind::Method,
            "Geometry::Circle#shift",
        ),
    ] {
        let found = symbol(&shard, qualified);
        assert_eq!(found.kind(), kind, "{qualified}");
        assert_eq!(found.display_name, display, "{qualified}");
    }
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids.len(), 16, "{ids:?}");
    for hidden in ["cache_key", "recompute", "label~3D"] {
        assert!(
            !ids.iter().any(|id| id.ends_with(hidden)),
            "{hidden} leaked: {ids:?}"
        );
    }
}

#[test]
fn signatures_docs_types_and_sources_come_from_yard() {
    let shard = shard("DX_FIXTURE_SHARD");
    let shape = symbol(&shard, "Geometry.Shape");
    assert_eq!(shape.signature_text, "class Geometry::Shape");
    assert_eq!(shape.doc_markdown, "A closed plane figure.");
    let scale = symbol(&shard, "Geometry.Shape.scale");
    assert_eq!(scale.signature_text, "def scale(factor)");
    assert_eq!(scale.doc_markdown, "Scales the shape by a factor.");
    assert_eq!(scale.params.len(), 1);
    assert_eq!(
        (
            scale.params[0].name.as_str(),
            scale.params[0].r#type.as_str(),
            scale.params[0].doc.as_str()
        ),
        (
            "factor",
            "Numeric",
            "the multiplier applied to every length."
        )
    );
    let returns = scale.returns.as_ref().expect("returns");
    assert_eq!(
        (returns.r#type.as_str(), returns.doc.as_str()),
        ("Shape", "a new, scaled shape.")
    );

    let area = symbol(&shard, "Geometry.Circle.area");
    assert_eq!(area.signature_text, "def area");
    assert_eq!(area.doc_markdown, "Computes the area of the circle.");
    assert!(area.params.is_empty());
    assert_eq!(area.returns.as_ref().expect("returns").r#type, "Float");
    let source = area.source.as_ref().expect("a source");
    assert_eq!(
        (source.file.as_str(), source.line),
        (SHAPES, line_of("DX_FIXTURE_SHAPES_RB", "def area"))
    );
    assert!(!source.generated);
    assert_eq!(
        area.relations.as_ref().expect("relations").member_of,
        vec![format!("{PREFIX}Geometry.Circle")]
    );

    let circle = symbol(&shard, "Geometry.Circle");
    assert_eq!(
        circle.signature_text,
        "class Geometry::Circle < Geometry::Shape"
    );
    assert_eq!(
        circle.relations.as_ref().expect("relations").implements,
        vec![format!("{PREFIX}Geometry.Sized")]
    );
    assert_eq!(
        symbol(&shard, "Geometry.Circle.self.unit").signature_text,
        "def self.unit"
    );
    assert_eq!(
        symbol(&shard, "Geometry.Circle.radius").signature_text,
        "attr_reader :radius"
    );
    assert_eq!(
        symbol(&shard, "Geometry.Circle.label").signature_text,
        "attr_accessor :label"
    );
    assert_eq!(
        symbol(&shard, "Geometry.DIMENSIONS").signature_text,
        "Geometry::DIMENSIONS = 2"
    );
    let module = symbol(&shard, "Geometry");
    let source = module.source.as_ref().expect("a source");
    assert_eq!(
        (source.file.as_str(), source.line),
        (
            GEOMETRY,
            line_of("DX_FIXTURE_GEOMETRY_RB", "module Geometry")
        )
    );
    for symbol in &shard.symbols {
        assert_eq!(
            extension(symbol, "ruby.config"),
            Some("ruby=3.4.9;yard=0.9.45;format=1"),
            "{}",
            symbol.id
        );
    }
}

#[test]
fn undocumented_and_untyped_members_are_marked_incomplete() {
    let shard = shard("DX_FIXTURE_SHARD");
    assert_eq!(
        extension(
            symbol(&shard, "Geometry.Circle.diameter"),
            "ruby.incomplete"
        ),
        Some("doc;return_type")
    );
    assert_eq!(
        extension(symbol(&shard, "Geometry.Circle.shift"), "ruby.incomplete"),
        Some("param_types=offset")
    );
    assert_eq!(
        extension(symbol(&shard, "Geometry.Circle.area"), "ruby.incomplete"),
        None
    );
}

#[test]
fn a_dependent_library_links_into_its_dependency() {
    let ext = shard("DX_FIXTURE_EXT_SHARD");
    validate_shard(&ext).expect("validate_shard passes");
    assert_eq!(ext.package, "geometry_ext");
    let square = find(&ext, "ruby:geometry_ext:GeometryExt.Square").expect("Square");
    assert_eq!(
        square.signature_text,
        "class GeometryExt::Square < Geometry::Shape"
    );
    assert_eq!(
        square.relations.as_ref().expect("relations").implements,
        vec![format!("{PREFIX}Geometry.Sized")]
    );
    let source = square.source.as_ref().expect("a source");
    assert_eq!(
        (source.file.as_str(), source.line),
        (
            "tools/docs/ruby/fixture/ext/square.rb",
            line_of("DX_FIXTURE_SQUARE_RB", "class Square")
        )
    );
    assert!(
        ext.symbols
            .iter()
            .all(|s| s.id.starts_with("ruby:geometry_ext:GeometryExt")),
        "the dependency leaked into the shard"
    );
}

#[test]
fn a_changed_method_changes_only_that_symbol() {
    let base = shard("DX_FIXTURE_SHARD");
    let changed = shard("DX_FIXTURE_CHANGED_SHARD");
    assert_eq!(base.symbols.len(), changed.symbols.len());
    for (before, after) in base.symbols.iter().zip(&changed.symbols) {
        assert_eq!(before.id, after.id);
        let mut after = after.clone();
        if let Some(source) = after.source.as_mut() {
            source.file = source.file.replace("/changed/", "/lib/");
        }
        if before.id.ends_with("Geometry.Circle.area") {
            assert_ne!(before, &after);
            assert_eq!(after.signature_text, "def area(precision = 2)");
            assert_eq!(
                after.doc_markdown,
                "Computes the area of the circle, rounded to two decimals."
            );
        } else {
            assert_eq!(before, &after, "{}", before.id);
        }
    }
}

#[test]
fn crlf_sources_yield_the_same_shard() {
    let base = shard("DX_FIXTURE_SHARD");
    let crlf = shard("DX_FIXTURE_CRLF_SHARD");
    assert_eq!(base.symbols.len(), crlf.symbols.len());
    for (before, after) in base.symbols.iter().zip(&crlf.symbols) {
        let mut after = after.clone();
        if let Some(source) = after.source.as_mut() {
            if source.file == CRLF_SHAPES {
                source.file = SHAPES.to_owned();
            }
        }
        assert_eq!(before, &after, "{}", before.id);
    }
}

struct Driver {
    dir: PathBuf,
    root: PathBuf,
}

impl Driver {
    fn new(name: &str) -> Driver {
        let dir = dx_testing::mkscratch(name).expect("scratch");
        let broken = runfile("DX_TESTDATA_BROKEN");
        let root = broken
            .ancestors()
            .nth(6)
            .expect("the runfiles root")
            .to_path_buf();
        Driver { dir, root }
    }

    fn manifest(&self, name: &str, files: &[&str]) -> PathBuf {
        let path = self.dir.join(name);
        let text: String = files
            .iter()
            .map(|rel| format!("{}\t{rel}\t0\n", self.root.join(rel).display()))
            .collect();
        std::fs::write(&path, text).expect("write the manifest");
        path
    }

    fn run(
        &self,
        yard_lib: &Path,
        own: &Path,
        cwd: &Path,
        home: &Path,
        plugin_lib: Option<&Path>,
    ) -> (i32, String) {
        let srcs = self.manifest("context.tsv", &[]);
        let deps = self.manifest("deps.tsv", &[]);
        let mut command = Command::new(runfile("DX_RUBY"));
        command.arg("--disable-gems").arg("-I").arg(yard_lib);
        if let Some(lib) = plugin_lib {
            command.arg("-I").arg(lib);
        }
        let output = command
            .arg(runfile("DX_YARD_DUMP"))
            .arg("--own")
            .arg(own)
            .arg("--srcs")
            .arg(&srcs)
            .arg("--deps")
            .arg(&deps)
            .arg("--scratch")
            .arg(self.dir.join("home"))
            .arg("--out")
            .arg(self.dir.join("out.json"))
            .current_dir(cwd)
            .env_clear()
            .env("HOME", home)
            .env("PATH", "/usr/bin:/bin")
            .output()
            .expect("run ruby");
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    fn out(&self) -> PathBuf {
        self.dir.join("out.json")
    }
}

fn yard_lib() -> PathBuf {
    runfile("DX_YARD_RB").parent().expect("lib").to_path_buf()
}

fn testdata() -> PathBuf {
    runfile("DX_TESTDATA_BROKEN")
        .parent()
        .expect("testdata")
        .to_path_buf()
}

fn bytes_of(path: &Path) -> Vec<u8> {
    std::fs::read(path).expect("read the source")
}

#[test]
fn the_driver_fails_on_a_syntax_error_and_writes_nothing() {
    let driver = Driver::new("ruby-broken");
    let rel = "tools/docs/ruby/fixture/testdata/broken.rb";
    let before = bytes_of(&driver.root.join(rel));
    let own = driver.manifest("own.tsv", &[rel]);
    let (code, stderr) = driver.run(&yard_lib(), &own, &driver.dir, &driver.dir, None);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("Syntax error"), "{stderr}");
    assert!(!driver.out().exists());
    assert_eq!(bytes_of(&driver.root.join(rel)), before);
}

#[test]
fn the_driver_fails_on_a_missing_manifest_or_source() {
    let driver = Driver::new("ruby-missing");
    let missing = driver.dir.join("no-such.tsv");
    let (code, stderr) = driver.run(&yard_lib(), &missing, &driver.dir, &driver.dir, None);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("cannot read manifest"), "{stderr}");
    let own = driver.manifest("own.tsv", &["tools/docs/ruby/fixture/lib/nope.rb"]);
    let (code, stderr) = driver.run(&yard_lib(), &own, &driver.dir, &driver.dir, None);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("does not exist"), "{stderr}");
    assert!(!driver.out().exists());
}

#[test]
fn the_driver_fails_without_yard() {
    let driver = Driver::new("ruby-noyard");
    let empty = driver.dir.join("empty-lib");
    std::fs::create_dir_all(&empty).expect("mkdir");
    let own = driver.manifest("own.tsv", &[SHAPES]);
    let (code, stderr) = driver.run(&empty, &own, &driver.dir, &driver.dir, None);
    assert_ne!(code, 0, "{stderr}");
    assert!(stderr.contains("yard"), "{stderr}");
    assert!(!driver.out().exists());
}

#[test]
fn a_private_only_library_fails_in_the_converter() {
    let driver = Driver::new("ruby-private");
    let own = driver.manifest(
        "own.tsv",
        &["tools/docs/ruby/fixture/testdata/private_only.rb"],
    );
    let (code, stderr) = driver.run(&yard_lib(), &own, &driver.dir, &driver.dir, None);
    assert_eq!(code, 0, "{stderr}");
    let shard = driver.dir.join("out.ir.pb");
    let (code, stderr) = extract(&driver.out(), &shard);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("no public Ruby API"), "{stderr}");
    assert!(!shard.exists());
}

#[test]
fn plugins_yardopts_and_home_config_are_never_loaded() {
    let driver = Driver::new("ruby-plugin");
    let plugin = testdata().join("plugin");
    assert!(plugin.join(".yardopts").exists());
    assert!(plugin.join("home/.yard/config").exists());
    let own = driver.manifest("own.tsv", &[GEOMETRY, SHAPES]);
    let sources = [GEOMETRY, SHAPES].map(|rel| bytes_of(&driver.root.join(rel)));
    let (code, stderr) = driver.run(
        &yard_lib(),
        &own,
        &plugin,
        &plugin.join("home"),
        Some(&plugin.join("lib")),
    );
    assert_eq!(code, 0, "{stderr}");
    assert!(!stderr.contains("yard-evil"), "{stderr}");
    let mut written: Vec<String> = std::fs::read_dir(&driver.dir)
        .expect("list")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    written.sort();
    assert_eq!(
        written,
        ["context.tsv", "deps.tsv", "home", "out.json", "own.tsv"]
    );
    assert_eq!(
        [GEOMETRY, SHAPES].map(|rel| bytes_of(&driver.root.join(rel))),
        sources
    );
    let json = std::fs::read_to_string(driver.out()).expect("json");
    let fixture = std::fs::read_to_string(runfile("DX_FIXTURE_JSON")).expect("fixture json");
    assert_eq!(json, fixture);
}

fn extract(json: &Path, out: &Path) -> (i32, String) {
    let output = Command::new(runfile("DX_RUBY_EXTRACT"))
        .arg("--json")
        .arg(json)
        .arg("--package")
        .arg("geometry")
        .arg("--out")
        .arg(out)
        .output()
        .expect("run ruby_extract");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn the_converter_rejects_a_new_format_and_truncated_json() {
    let dir = dx_testing::mkscratch("ruby-extract").expect("scratch");
    let json = std::fs::read_to_string(runfile("DX_FIXTURE_JSON")).expect("driver json");
    assert!(json.contains("\"format\": 1,"));
    let out = dir.join("shard.ir.pb");
    for (name, text, needle) in [
        (
            "new.json",
            json.replace("\"format\": 1,", "\"format\": 2,"),
            "format 2 is not supported",
        ),
        ("cut.json", json[..json.len() / 2].to_owned(), "malformed"),
    ] {
        std::fs::write(dir.join(name), text).expect("write");
        let (code, stderr) = extract(&dir.join(name), &out);
        assert_eq!(code, 1, "{stderr}");
        assert!(stderr.contains(needle), "{stderr}");
        assert!(!out.exists());
    }
    std::fs::write(dir.join("ok.json"), &json).expect("write");
    let (code, stderr) = extract(&dir.join("ok.json"), &out);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        std::fs::read(&out).expect("shard"),
        std::fs::read(runfile("DX_FIXTURE_SHARD")).expect("fixture")
    );
}
