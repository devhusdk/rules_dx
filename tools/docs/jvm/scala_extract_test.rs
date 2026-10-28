//! Checks the Scala fixture shards and the Scaladoc adapter's and converter's failure handling.

use std::path::{Path, PathBuf};
use std::process::Command;

use documentation_ir::proto::SymbolKind;
use documentation_ir::validate_shard;
use jvm_test_support::{ids, line_of, runfile, shard, symbol};

const SHAPES: &str = "scala:shapes:dx.fixture.shapes.";

#[test]
fn the_shapes_shard_maps_every_public_symbol() {
    let shard = shard("DX_SHAPES_SHARD");
    validate_shard(&shard).expect("validate_shard passes");
    assert_eq!(shard.language, "scala");
    assert_eq!(shard.package, "shapes");
    let mut expected = vec![
        ("Circle", SymbolKind::Class),
        ("Circle.Circle~28Double~29", SymbolKind::Method),
        ("Circle.Size", SymbolKind::TypeAlias),
        ("Circle.area~28~29", SymbolKind::Function),
        ("Circle.radius", SymbolKind::Property),
        ("Circle.scale~28Double~29", SymbolKind::Function),
        ("Circle.scale~28Double~2CDouble~29", SymbolKind::Function),
        ("Point", SymbolKind::Class),
        ("Point.Point~28Double~2CDouble~29", SymbolKind::Method),
        ("Point.x", SymbolKind::Property),
        ("Point.y", SymbolKind::Property),
        ("Point~24", SymbolKind::Class),
        ("Point~24.origin~28~29", SymbolKind::Function),
        ("Shape", SymbolKind::Interface),
        ("Shape.area~28~29", SymbolKind::Function),
    ];
    for (suffix, kind) in &expected {
        let id = format!("{SHAPES}{suffix}");
        assert_eq!(symbol(&shard, &id).kind, *kind as i32, "{id}");
    }
    let mut actual = ids(&shard);
    actual.sort_unstable();
    let mut wanted: Vec<String> = expected
        .drain(..)
        .map(|(suffix, _)| format!("{SHAPES}{suffix}"))
        .collect();
    wanted.sort_unstable();
    assert_eq!(
        actual, wanted,
        "private, inherited and synthetic members leaked"
    );
}

#[test]
fn a_function_carries_its_signature_docs_and_source() {
    let shard = shard("DX_SHAPES_SHARD");
    let one = symbol(&shard, &format!("{SHAPES}Circle.scale~28Double~29"));
    let two = symbol(
        &shard,
        &format!("{SHAPES}Circle.scale~28Double~2CDouble~29"),
    );
    assert_eq!(one.signature_text, "def scale(factor: Double): Circle");
    assert_eq!(one.doc_markdown, "Scales the radius by one factor.");
    assert_eq!(one.params.len(), 1);
    assert_eq!(one.params[0].doc, "the factor to multiply by");
    assert_eq!(
        two.signature_text,
        "def scale(horizontal: Double, vertical: Double): Circle"
    );
    assert!(
        two.doc_markdown
            .contains("like [scale](dx:scala:shapes:dx.fixture.shapes.Circle.scale~28Double~29)."),
        "{}",
        two.doc_markdown
    );
    assert_eq!(two.params.len(), 2);
    assert_eq!(two.params[1].name, "vertical");
    assert_eq!(two.params[1].r#type, "Double");
    assert_eq!(two.params[1].doc, "the second factor");
    assert_eq!(
        two.relations.as_ref().expect("relations").member_of,
        vec![format!("{SHAPES}Circle")]
    );
    assert_eq!(
        one.source.as_ref().expect("a source").line,
        line_of("DX_CIRCLE_SCALA", "def scale(factor: Double)")
    );
    assert_eq!(
        two.source.as_ref().expect("a source").line,
        line_of("DX_CIRCLE_SCALA", "def scale(horizontal: Double")
    );
    let circle = symbol(&shard, &format!("{SHAPES}Circle"));
    assert_eq!(
        circle.source.as_ref().expect("a source").line,
        line_of("DX_CIRCLE_SCALA", "class Circle")
    );
}

#[test]
fn a_class_and_its_companion_get_distinct_ids() {
    let shard = shard("DX_SHAPES_SHARD");
    let class = symbol(&shard, &format!("{SHAPES}Point"));
    let object = symbol(&shard, &format!("{SHAPES}Point~24"));
    assert_eq!(class.signature_text, "case class Point");
    assert_eq!(object.signature_text, "object Point");
    assert_eq!(
        object.source.as_ref().expect("a source").line,
        line_of("DX_CIRCLE_SCALA", "object Point")
    );
    for synthetic in ["copy", "productArity", "canEqual", "apply", "unapply"] {
        assert!(
            shard.symbols.iter().all(|s| !s.id.contains(synthetic)),
            "{synthetic} leaked into {:?}",
            ids(&shard)
        );
    }
}

#[test]
fn a_dependent_library_links_into_its_dependency() {
    let paint = shard("DX_PAINT_SHARD");
    validate_shard(&paint).expect("validate_shard passes");
    let square = symbol(&paint, "scala:paint:dx.fixture.paint.Square");
    assert_eq!(
        square.relations.as_ref().expect("relations").implements,
        vec![format!("{SHAPES}Shape")]
    );
    assert_eq!(
        square.doc_markdown,
        "A square that is painted like a [Circle](dx:scala:shapes:dx.fixture.shapes.Circle)."
    );
}

#[test]
fn without_the_dependency_shard_links_degrade_to_code() {
    let paint = shard("DX_PAINT_NODEP_SHARD");
    validate_shard(&paint).expect("validate_shard passes");
    let square = symbol(&paint, "scala:paint:dx.fixture.paint.Square");
    assert!(square
        .relations
        .as_ref()
        .is_none_or(|r| r.implements.is_empty()));
    assert_eq!(
        square.doc_markdown,
        "A square that is painted like a `Circle`."
    );
}

struct Driver {
    _scratch: dx_test_scratch::TempDir,
    dir: PathBuf,
}

impl Driver {
    fn new(name: &str) -> Driver {
        let scratch = dx_test_scratch::scratch(name);
        let dir = scratch.path().to_path_buf();
        Driver {
            _scratch: scratch,
            dir,
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
        let output = Command::new(runfile("DX_SCALADOC_EXTRACT"))
            .arg("--srcs-manifest")
            .arg(manifest)
            .arg("--classpath-file")
            .arg(classpath)
            .arg("--release")
            .arg("11")
            .arg("--out")
            .arg(self.out())
            .output()
            .expect("run scaladoc_extract");
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }
}

#[test]
fn the_adapter_fails_on_a_type_error_and_writes_nothing() {
    let driver = Driver::new("scala-broken");
    let source = driver.source(
        "Broken.scala",
        "package broken\n\n/** Broken. */\nobject Broken {\n  def value: Missing = ???\n}\n",
    );
    let (code, stderr) = driver.run(&driver.manifest(&[source]), &driver.classpath(&[]));
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("Missing"), "{stderr}");
    assert!(!driver.out().exists());
}

#[test]
fn the_adapter_fails_on_a_missing_source_or_classpath_jar() {
    let driver = Driver::new("scala-missing");
    let nope = driver.dir.join("Nope.scala");
    let (code, stderr) = driver.run(&driver.manifest(&[nope]), &driver.classpath(&[]));
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("missing source"), "{stderr}");
    let manifest = driver.manifest(&[runfile("DX_SHAPE_SCALA")]);
    let (code, stderr) = driver.run(&manifest, &driver.classpath(&["no-such.jar"]));
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("missing classpath entry"), "{stderr}");
    assert!(!driver.out().exists());
}

#[test]
fn an_empty_public_api_fails_and_writes_nothing() {
    let driver = Driver::new("scala-empty");
    let source = driver.source(
        "Hidden.scala",
        "package hidden\n\nprivate[hidden] class Hidden\n",
    );
    let manifest = driver.manifest(&[source]);
    let (code, stderr) = driver.run(&manifest, &driver.classpath(&[]));
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("no public declarations"), "{stderr}");
    assert!(!driver.out().exists());
}
