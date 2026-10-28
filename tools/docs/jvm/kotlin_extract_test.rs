//! Checks the Kotlin fixture shards and the Dokka adapter's and converter's failure handling.

use std::path::{Path, PathBuf};
use std::process::Command;

use documentation_ir::proto::SymbolKind;
use documentation_ir::validate_shard;
use jvm_test_support::{find, ids, line_of, runfile, shard, symbol};

const SHAPES: &str = "kotlin:shapes:dx.fixture.shapes.";

#[test]
fn the_shapes_shard_maps_every_public_symbol() {
    let shard = shard("DX_SHAPES_SHARD");
    validate_shard(&shard).expect("validate_shard passes");
    assert_eq!(shard.language, "kotlin");
    assert_eq!(shard.package, "shapes");
    for (suffix, kind) in [
        ("Circle", SymbolKind::Class),
        ("Circle.Companion", SymbolKind::Class),
        ("Circle.Companion.MAX_RADIUS", SymbolKind::Property),
        ("Circle.Size", SymbolKind::Enum),
        ("Circle.Size.SMALL", SymbolKind::EnumVariant),
        ("Circle.Size.LARGE", SymbolKind::EnumVariant),
        ("Circle.scale~28Double~29", SymbolKind::Function),
        ("Circle.scale~28Double~2CDouble~29", SymbolKind::Function),
        ("Shape", SymbolKind::Interface),
        ("Registry", SymbolKind::Class),
        ("grow~28Circle~2CDouble~29", SymbolKind::Function),
    ] {
        let id = format!("{SHAPES}{suffix}");
        assert_eq!(symbol(&shard, &id).kind, kind as i32, "{id}");
    }
    for hidden in [
        "hidden",
        "secret",
        "inside",
        "hiddenHelper",
        "values",
        "entries",
    ] {
        assert!(
            shard.symbols.iter().all(|s| !s.id.contains(hidden)),
            "{hidden} leaked into {:?}",
            ids(&shard)
        );
    }
}

#[test]
fn extension_properties_are_keyed_by_their_receiver() {
    let shard = shard("DX_SHAPES_SHARD");
    let on_circle = ids(&shard)
        .into_iter()
        .filter(|id| id.contains(".diameter"))
        .collect::<Vec<_>>();
    assert_eq!(on_circle.len(), 2, "{on_circle:?}");
    let circle = symbol(&shard, &format!("{SHAPES}diameter~40Circle"));
    assert_eq!(circle.signature_text, "val Circle.diameter: Double");
    assert_eq!(circle.doc_markdown, "The diameter of a circle.");
    assert!(find(&shard, &format!("{SHAPES}diameter~40Shape")).is_some());
}

#[test]
fn a_function_carries_its_signature_docs_and_source() {
    let shard = shard("DX_SHAPES_SHARD");
    let one = symbol(&shard, &format!("{SHAPES}Circle.scale~28Double~29"));
    let two = symbol(
        &shard,
        &format!("{SHAPES}Circle.scale~28Double~2CDouble~29"),
    );
    assert_eq!(one.signature_text, "fun scale(factor: Double): Circle");
    assert_eq!(
        one.doc_markdown,
        "Scales the radius by one factor.\n\n**Throws**\n\n- `IllegalArgumentException`: if the factor is negative"
    );
    assert!(
        two.doc_markdown.contains(
            "like [scale](dx:kotlin:shapes:dx.fixture.shapes.Circle.scale~28Double~2CDouble~29)."
        ),
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
        line_of("DX_CIRCLE_KT", "fun scale(factor: Double)")
    );
    let companion = symbol(&shard, &format!("{SHAPES}Circle.Companion"));
    assert_eq!(
        companion.doc_markdown,
        "Holds the constants and factories of circles."
    );
}

#[test]
fn a_dependent_library_links_into_its_dependency() {
    let paint = shard("DX_PAINT_SHARD");
    validate_shard(&paint).expect("validate_shard passes");
    let square = symbol(&paint, "kotlin:paint:dx.fixture.paint.Square");
    assert_eq!(
        square.relations.as_ref().expect("relations").implements,
        vec![format!("{SHAPES}Shape")]
    );
    assert_eq!(
        square.doc_markdown,
        "A square that is painted like a [Circle](dx:kotlin:shapes:dx.fixture.shapes.Circle)."
    );
}

#[test]
fn without_the_dependency_shard_links_degrade_to_code() {
    let paint = shard("DX_PAINT_NODEP_SHARD");
    validate_shard(&paint).expect("validate_shard passes");
    let square = symbol(&paint, "kotlin:paint:dx.fixture.paint.Square");
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
        let output = Command::new(runfile("DX_DOKKA_EXTRACT"))
            .arg("--srcs-manifest")
            .arg(manifest)
            .arg("--classpath-file")
            .arg(classpath)
            .arg("--release")
            .arg("11")
            .arg("--out")
            .arg(self.out())
            .output()
            .expect("run dokka_extract");
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }
}

#[test]
fn the_adapter_fails_on_an_unresolved_public_type_and_writes_nothing() {
    let driver = Driver::new("kotlin-broken");
    let source = driver.source(
        "Broken.kt",
        "package broken\n\n/** Broken. */\nfun value(): Missing = TODO()\n",
    );
    let (code, stderr) = driver.run(&driver.manifest(&[source]), &driver.classpath(&[]));
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("Missing"), "{stderr}");
    assert!(!driver.out().exists());
}

#[test]
fn the_adapter_fails_on_a_missing_source_or_classpath_jar() {
    let driver = Driver::new("kotlin-missing");
    let nope = driver.dir.join("Nope.kt");
    let (code, stderr) = driver.run(&driver.manifest(&[nope]), &driver.classpath(&[]));
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("missing source"), "{stderr}");
    let manifest = driver.manifest(&[runfile("DX_SHAPE_KT")]);
    let (code, stderr) = driver.run(&manifest, &driver.classpath(&["no-such.jar"]));
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("missing classpath entry"), "{stderr}");
    assert!(!driver.out().exists());
}

#[test]
fn an_empty_public_api_fails_and_writes_nothing() {
    let driver = Driver::new("kotlin-empty");
    let source = driver.source("Hidden.kt", "package hidden\n\ninternal class Hidden\n");
    let manifest = driver.manifest(&[source]);
    let (code, stderr) = driver.run(&manifest, &driver.classpath(&[]));
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("no public declarations"), "{stderr}");
    assert!(!driver.out().exists());
}
