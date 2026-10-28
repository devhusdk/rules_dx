//! Checks the Java fixture shards and the Javadoc adapter's and converter's failure handling.

use std::path::{Path, PathBuf};
use std::process::Command;

use documentation_ir::proto::{DocIr, Symbol, SymbolKind};
use documentation_ir::{decode_shard, validate_shard};

const SHAPES: &str = "java:shapes:dx.fixture.shapes.";
const CIRCLE: &str = "tools/docs/jvm/fixture/java/dx/fixture/shapes/Circle.java";

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

fn symbol<'a>(shard: &'a DocIr, id: &str) -> &'a Symbol {
    find(shard, id).unwrap_or_else(|| panic!("{id} is missing"))
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
fn the_shapes_shard_maps_every_public_symbol() {
    let shard = shard("DX_SHAPES_SHARD");
    validate_shard(&shard).expect("validate_shard passes");
    assert_eq!(shard.language, "java");
    assert_eq!(shard.package, "shapes");
    for (suffix, kind) in [
        ("", SymbolKind::Module),
        ("Circle", SymbolKind::Class),
        ("Circle.Circle~28double~29", SymbolKind::Method),
        ("Circle.MAX_RADIUS", SymbolKind::Constant),
        ("Circle.Size", SymbolKind::Enum),
        ("Circle.Size.SMALL", SymbolKind::EnumVariant),
        ("Circle.Size.LARGE", SymbolKind::EnumVariant),
        ("Circle.area~28~29", SymbolKind::Method),
        ("Circle.scale~28double~29", SymbolKind::Method),
        ("Circle.scale~28double~2Cdouble~29", SymbolKind::Method),
        ("Circle.size~28~29", SymbolKind::Method),
        ("Shape", SymbolKind::Interface),
        ("Shape.area~28~29", SymbolKind::Method),
    ] {
        let id = if suffix.is_empty() {
            "java:shapes:dx.fixture.shapes".to_owned()
        } else {
            format!("{SHAPES}{suffix}")
        };
        assert_eq!(symbol(&shard, &id).kind, kind as i32, "{id}");
    }
    assert_eq!(shard.symbols.len(), 13, "{:?}", ids(&shard));
    for hidden in ["hidden", "secret", "Internal", "inside"] {
        assert!(
            shard.symbols.iter().all(|s| !s.id.contains(hidden)),
            "{hidden} leaked into {:?}",
            ids(&shard)
        );
    }
}

fn ids(shard: &DocIr) -> Vec<&str> {
    shard.symbols.iter().map(|s| s.id.as_str()).collect()
}

#[test]
fn signatures_docs_overloads_and_sources_come_from_javadoc() {
    let shard = shard("DX_SHAPES_SHARD");
    let circle = symbol(&shard, "java:shapes:dx.fixture.shapes.Circle");
    assert_eq!(
        circle.signature_text,
        "public class Circle implements Shape"
    );
    assert_eq!(circle.display_name, "dx.fixture.shapes.Circle");
    assert!(
        circle
            .doc_markdown
            .starts_with("A circle given by its radius, drawn with `radius` as `double`.\n\nUse "),
        "{}",
        circle.doc_markdown
    );
    let link = "[Circle.scale(double, double)](dx:java:shapes:dx.fixture.shapes.Circle.scale~28double~2Cdouble~29)";
    assert!(
        circle.doc_markdown.contains(link),
        "{}",
        circle.doc_markdown
    );
    assert!(
        circle.doc_markdown.contains("&lt;script&gt;") && !circle.doc_markdown.contains("<script>"),
        "{}",
        circle.doc_markdown
    );
    assert!(
        circle
            .doc_markdown
            .contains("\n\n- round\n- closed\n\n**Since:** 1.0"),
        "{}",
        circle.doc_markdown
    );
    let source = circle.source.as_ref().expect("a source");
    assert_eq!(
        (source.file.as_str(), source.line, source.generated),
        (
            CIRCLE,
            line_of("DX_CIRCLE_JAVA", "public class Circle"),
            false
        )
    );

    let one = symbol(&shard, &format!("{SHAPES}Circle.scale~28double~29"));
    let two = symbol(
        &shard,
        &format!("{SHAPES}Circle.scale~28double~2Cdouble~29"),
    );
    assert_eq!(one.signature_text, "public Circle scale(double factor)");
    assert_eq!(
        two.signature_text,
        "public Circle scale(double horizontal, double vertical)"
    );
    assert_eq!(one.display_name, "dx.fixture.shapes.Circle.scale(double)");
    assert_eq!(
        two.display_name,
        "dx.fixture.shapes.Circle.scale(double, double)"
    );
    assert_eq!(
        one.doc_markdown,
        "Scales the radius by one factor.\n\n**Throws**\n\n- `IllegalArgumentException`: if the factor is negative"
    );
    assert!(
        two.doc_markdown.contains(
            "like [scale(double)](dx:java:shapes:dx.fixture.shapes.Circle.scale~28double~29)."
        ),
        "{}",
        two.doc_markdown
    );
    assert_eq!(two.params.len(), 2);
    assert_eq!(two.params[1].name, "vertical");
    assert_eq!(two.params[1].r#type, "double");
    assert_eq!(two.params[1].doc, "the second factor");
    assert_eq!(two.returns.as_ref().expect("returns").r#type, "Circle");
    assert_eq!(
        two.relations.as_ref().expect("relations").member_of,
        vec![format!("{SHAPES}Circle")]
    );
    assert_eq!(
        two.source.as_ref().expect("a source").line,
        line_of("DX_CIRCLE_JAVA", "public Circle scale(double horizontal")
    );

    let pi = symbol(&shard, &format!("{SHAPES}Circle.MAX_RADIUS"));
    assert_eq!(
        pi.signature_text,
        "public static final double MAX_RADIUS = 100.0"
    );
    let ctor = symbol(&shard, &format!("{SHAPES}Circle.Circle~28double~29"));
    assert_eq!(ctor.signature_text, "public Circle(double radius)");
    let nested = symbol(&shard, &format!("{SHAPES}Circle.Size"));
    assert_eq!(
        nested.relations.as_ref().expect("relations").member_of,
        vec![format!("{SHAPES}Circle")]
    );
    let package = symbol(&shard, "java:shapes:dx.fixture.shapes");
    assert_eq!(package.doc_markdown, "Plane figures with an area.");
}

#[test]
fn an_undocumented_override_inherits_the_documentation_of_its_declaration() {
    let shard = shard("DX_SHAPES_SHARD");
    let area = symbol(&shard, &format!("{SHAPES}Circle.area~28~29"));
    assert_eq!(area.doc_markdown, "Returns the area of this shape.");
    let returns = area.returns.as_ref().expect("returns");
    assert_eq!(returns.doc, "the area in square units");
}

#[test]
fn a_dependent_library_links_into_its_dependency() {
    let paint = shard("DX_PAINT_SHARD");
    validate_shard(&paint).expect("validate_shard passes");
    assert_eq!(paint.package, "paint");
    let square = symbol(&paint, "java:paint:dx.fixture.paint.Square");
    assert_eq!(
        square.signature_text,
        "public final class Square implements Shape"
    );
    assert_eq!(
        square.relations.as_ref().expect("relations").implements,
        vec![format!("{SHAPES}Shape")]
    );
    assert_eq!(
        square.doc_markdown,
        "A square that can be painted like [Circle](dx:java:shapes:dx.fixture.shapes.Circle)."
    );
}

#[test]
fn without_the_dependency_shard_links_degrade_to_code() {
    let paint = shard("DX_PAINT_NODEP_SHARD");
    validate_shard(&paint).expect("validate_shard passes");
    let square = symbol(&paint, "java:paint:dx.fixture.paint.Square");
    assert!(square
        .relations
        .as_ref()
        .is_none_or(|r| r.implements.is_empty()));
    assert_eq!(
        square.doc_markdown,
        "A square that can be painted like `Circle`."
    );
}

struct Driver {
    _scratch: dx_test_scratch::TempDir,
    dir: PathBuf,
    root: PathBuf,
}

impl Driver {
    fn new(name: &str) -> Driver {
        let scratch = dx_test_scratch::scratch(name);
        let dir = scratch.path().to_path_buf();
        let root = runfile("DX_CIRCLE_JAVA")
            .ancestors()
            .nth(CIRCLE.matches('/').count() + 1)
            .expect("the repo root")
            .to_path_buf();
        Driver {
            _scratch: scratch,
            dir,
            root,
        }
    }

    fn manifest(&self, files: &[PathBuf]) -> PathBuf {
        let path = self.dir.join("srcs.tsv");
        let text: String = files
            .iter()
            .map(|file| format!("{}\t{}\t0\n", file.display(), file.display()))
            .collect();
        std::fs::write(&path, text).expect("write the manifest");
        path
    }

    fn classpath(&self, jars: &[&str]) -> PathBuf {
        let path = self.dir.join("classpath.txt");
        let text: String = jars.iter().map(|jar| format!("{jar}\n")).collect();
        std::fs::write(&path, text).expect("write the classpath");
        path
    }

    fn source(&self, name: &str, text: &str) -> PathBuf {
        dx_test_scratch::write(&self.dir, name, text);
        self.dir.join(name)
    }

    fn out(&self) -> PathBuf {
        self.dir.join("out.api.json")
    }

    fn run(&self, manifest: &Path, classpath: &Path) -> (i32, String) {
        let output = Command::new(runfile("DX_JAVADOC_EXTRACT"))
            .arg("--srcs-manifest")
            .arg(manifest)
            .arg("--classpath-file")
            .arg(classpath)
            .arg("--release")
            .arg("11")
            .arg("--out")
            .arg(self.out())
            .output()
            .expect("run javadoc_extract");
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }
}

#[test]
fn the_adapter_documents_the_fixture_sources() {
    let driver = Driver::new("java-ok");
    let circle = runfile("DX_CIRCLE_JAVA");
    let shape = runfile("DX_SHAPE_JAVA");
    let manifest = driver.manifest(&[circle, shape]);
    let (code, stderr) = driver.run(&manifest, &driver.classpath(&[]));
    assert_eq!(code, 0, "{stderr}");
    let json = std::fs::read_to_string(driver.out()).expect("the record file");
    assert!(
        json.contains("\"qualified\":\"dx.fixture.shapes.Circle\""),
        "{json}"
    );
    assert!(driver.root.exists());
}

#[test]
fn the_adapter_fails_on_a_type_error_and_writes_nothing() {
    let driver = Driver::new("java-broken");
    let source = driver.source(
        "Broken.java",
        "package broken;\n\n/** Broken. */\npublic class Broken {\n  /** Bad. */\n  public Missing value() {\n    return null;\n  }\n}\n",
    );
    let (code, stderr) = driver.run(&driver.manifest(&[source]), &driver.classpath(&[]));
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("cannot find symbol"), "{stderr}");
    assert!(!driver.out().exists());
}

#[test]
fn the_adapter_fails_on_a_missing_source_or_classpath_jar() {
    let driver = Driver::new("java-missing");
    let nope = driver.dir.join("Nope.java");
    let (code, stderr) = driver.run(&driver.manifest(&[nope]), &driver.classpath(&[]));
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("missing source"), "{stderr}");
    let manifest = driver.manifest(&[runfile("DX_SHAPE_JAVA")]);
    let (code, stderr) = driver.run(&manifest, &driver.classpath(&["no-such.jar"]));
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("missing classpath entry"), "{stderr}");
    assert!(!driver.out().exists());
}

fn convert(json: &Path, manifest: &Path, out: &Path) -> (i32, String) {
    let output = Command::new(runfile("DX_JVM_CONVERT"))
        .arg("--json")
        .arg(json)
        .arg("--language")
        .arg("java")
        .arg("--package")
        .arg("shapes")
        .arg("--srcs-manifest")
        .arg(manifest)
        .arg("--out")
        .arg(out)
        .output()
        .expect("run jvm_convert");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn an_empty_public_api_fails_in_the_converter() {
    let driver = Driver::new("java-empty");
    let source = driver.source("Hidden.java", "package hidden;\n\nclass Hidden {}\n");
    let manifest = driver.manifest(&[source]);
    let (code, stderr) = driver.run(&manifest, &driver.classpath(&[]));
    assert_eq!(code, 0, "{stderr}");
    let shard = driver.dir.join("out.ir.pb");
    let (code, stderr) = convert(&driver.out(), &manifest, &shard);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("no public symbols"), "{stderr}");
    assert!(!shard.exists());
}

#[test]
fn the_converter_rejects_bad_records() {
    let driver = Driver::new("java-convert");
    let json = std::fs::read_to_string(runfile("DX_SHAPES_JSON")).expect("adapter json");
    assert!(json.starts_with("{\"format\":1,"), "{json}");
    let manifest = driver.dir.join("fixture.tsv");
    std::fs::write(
        &manifest,
        format!("{CIRCLE}\t{CIRCLE}\t0\ntools/docs/jvm/fixture/java/dx/fixture/shapes/Shape.java\ttools/docs/jvm/fixture/java/dx/fixture/shapes/Shape.java\t0\ntools/docs/jvm/fixture/java/dx/fixture/shapes/package-info.java\ttools/docs/jvm/fixture/java/dx/fixture/shapes/package-info.java\t0\n"),
    )
    .expect("write the manifest");
    let out = driver.dir.join("shard.ir.pb");
    for (name, text, needle) in [
        (
            "new.json",
            json.replacen("\"format\":1,", "\"format\":2,", 1),
            "format Some(2) is not supported",
        ),
        ("cut.json", json[..json.len() / 2].to_owned(), "malformed"),
        (
            "moved.json",
            json.replace("shapes/Shape.java", "shapes/Elsewhere.java"),
            "not in the srcs manifest",
        ),
    ] {
        std::fs::write(driver.dir.join(name), text).expect("write");
        let (code, stderr) = convert(&driver.dir.join(name), &manifest, &out);
        assert_eq!(code, 1, "{stderr}");
        assert!(stderr.contains(needle), "{stderr}");
        assert!(!out.exists());
    }
    std::fs::write(driver.dir.join("ok.json"), &json).expect("write");
    let (code, stderr) = convert(&driver.dir.join("ok.json"), &manifest, &out);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        std::fs::read(&out).expect("shard"),
        std::fs::read(runfile("DX_SHAPES_SHARD")).expect("fixture")
    );
}
