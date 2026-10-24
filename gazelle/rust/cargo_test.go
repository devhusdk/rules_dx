package rust

import (
	"strings"
	"testing"

	bzl "github.com/bazelbuild/buildtools/build"
)

func TestParseCargoManifest(t *testing.T) {
	manifest, err := parseCargoManifest("Cargo.toml", []byte(`
[package]
name = "demo-app"
edition = "2024"

[dependencies]
serde-json = "1"
renamed = { package = "actual-crate", version = "1" }

[dev-dependencies]
tempfile = "3"

[lib]
name = "demo_core"
path = "src/core.rs"

[[bin]]
name = "demo"
path = "cmd/demo.rs"

[[test]]
name = "integration-fast"
path = "spec/fast.rs"
harness = false
`))
	if err != nil {
		t.Fatal(err)
	}
	if manifest.packageName != "demo-app" || manifest.edition != "2024" || len(manifest.targets) != 3 {
		t.Fatalf("manifest = %+v", manifest)
	}
	if target := manifest.targets[2]; target.name != "integration_fast" || target.path != "spec/fast.rs" || target.harness {
		t.Errorf("test target = %+v", target)
	}
	if !manifest.normalDeps["serde_json"].external || !manifest.normalDeps["renamed"].external || !manifest.devDeps["tempfile"].external {
		t.Errorf("dependency scopes = normal %v dev %v", manifest.normalDeps, manifest.devDeps)
	}
}

func TestParseCargoDefaults(t *testing.T) {
	manifest, err := parseCargoManifest("Cargo.toml", []byte("[package]\nname = \"demo\" # comment\n"))
	if err != nil {
		t.Fatal(err)
	}
	if len(manifest.targets) != 0 || manifest.edition != "2021" {
		t.Fatalf("defaults = %+v", manifest)
	}
	if err := manifest.withImplicitTargets(map[string]bool{"src/lib.rs": true}, ""); err != nil {
		t.Fatal(err)
	}
	if len(manifest.targets) != 1 || manifest.targets[0].path != "src/lib.rs" {
		t.Errorf("implicit defaults = %+v", manifest.targets)
	}
}

func TestParseCargoFailsClosed(t *testing.T) {
	cases := []string{
		"[package]\nedition = \"2021\"\n",
		"[package]\nname = \"demo\"\n[[test]]\nname = \"x\"\nrequired-features = [\"slow\"]\n",
		"[package]\nname = \"demo\"\n[[bin]]\nname = \"same\"\n[[bin]]\nname = \"same\"\n",
		"[package]\nname = demo\n",
		"[package]\nname = \"demo\"\n[[test]]\nname = \"x\"\nharness = maybe\n",
		"[package]\nname = \"demo\"\n[lib]\nname = 1\n",
		"[package]\nname = \"demo\"\nnot-an-assignment\n",
		"[package]\nname = \"demo\"\nversion = 1\n",
		"[package]\nname = \"demo\"\nbuild = 42\n",
		"[package]\nname = \"demo\"\n[[example]]\nname = \"e\"\ntest = maybe\n",
		"[package]\nname = \"demo\"\n[[bin]]\nname = \"b\"\ntest = maybe\n",
		"[package]\nname = \"demo\"\n[[bench]]\nname = \"b\"\nbench = maybe\n",
		"[package]\nname = \"demo\"\n[[bin]]\nname = \"b\"\ncrate-type = [\"rlib\"]\n",
		"[package]\nname = \"demo\"\n[lib]\ncrate-type = 42\n",
		"[package]\nname = \"demo\"\n[lib]\ncrate-type = [42]\n",
		"[package]\nname = \"demo\"\n[[bin]]\nname = \"b\"\nproc-macro = true\n",
		"[package]\nname = \"demo\"\n[lib]\nproc-macro = maybe\n",
		"[package]\nname = \"demo\"\n[[bin]]\nname = \"b\"\nrequired-features = 42\n",
		"[package]\nname = \"---\"\n",
		"[package]\nname = \"demo\"\n[[example]]\n",
	}
	for _, source := range cases {
		if _, err := parseCargoManifest("Cargo.toml", []byte(source)); err == nil {
			t.Errorf("expected failure for %q", source)
		} else if !strings.Contains(err.Error(), "Cargo") && !strings.Contains(err.Error(), "package") {
			t.Errorf("missing context: %v", err)
		}
	}
}

func TestCargoCommentAndStringErrors(t *testing.T) {
	if got := stripTomlComment(`name = "a#b" # trailing`); strings.TrimSpace(got) != `name = "a#b"` {
		t.Errorf("comment stripping = %q", got)
	}
	if _, err := tomlString(`"unterminated`); err == nil {
		t.Error("unterminated string accepted")
	}
	if _, err := tomlString(`"bad\q"`); err == nil {
		t.Error("invalid escape accepted")
	}
	if got := stripTomlComment(`name = "a\\\"#b" # trailing`); !strings.Contains(got, "#b") {
		t.Errorf("escaped quote ended string: %q", got)
	}
}

func TestCargoImplicitTargets(t *testing.T) {
	manifest, err := parseCargoManifest("Cargo.toml", []byte("[package]\nname = \"demo-app\"\n"))
	if err != nil {
		t.Fatal(err)
	}
	files := map[string]bool{
		"crates/app/src/lib.rs":          true,
		"crates/app/src/main.rs":         true,
		"crates/app/tests/smoke.rs":      true,
		"crates/app/tests/common/mod.rs": true,
	}
	if err := manifest.withImplicitTargets(files, "crates/app"); err != nil {
		t.Fatal(err)
	}
	if len(manifest.targets) != 3 || manifest.targets[1].kind != binaryKind || manifest.targets[2].name != "smoke_test" {
		t.Errorf("implicit targets = %+v", manifest.targets)
	}
	lib, bin := manifest.targets[0], manifest.targets[1]
	if lib.kind != libraryKind || lib.name != "demo_app_lib" || lib.crate() != "demo_app" {
		t.Errorf("implicit lib = %+v", lib)
	}
	if bin.name != "demo_app" || bin.crate() != "demo_app" {
		t.Errorf("implicit bin = %+v", bin)
	}
	for _, file := range []string{"pkg/src/lib.rs", "pkg/src/main.rs"} {
		single, err := parseCargoManifest("Cargo.toml", []byte("[package]\nname = \"demo-app\"\n"))
		if err != nil {
			t.Fatal(err)
		}
		if err := single.withImplicitTargets(map[string]bool{file: true}, "pkg"); err != nil {
			t.Fatal(err)
		}
		if len(single.targets) != 1 || single.targets[0].name != "demo_app" || single.targets[0].crate() != "demo_app" {
			t.Errorf("%s implicit targets = %+v", file, single.targets)
		}
	}
}

func TestCargoExplicitTargetsSuppressImplicitDuplicates(t *testing.T) {
	manifest, err := parseCargoManifest("Cargo.toml", []byte("[package]\nname = \"demo\"\n[lib]\n[[bin]]\nname = \"tool\"\n[[test]]\nname = \"smoke\"\n"))
	if err != nil {
		t.Fatal(err)
	}
	files := map[string]bool{
		"pkg/src/lib.rs":     true,
		"pkg/src/main.rs":    true,
		"pkg/tests/smoke.rs": true,
	}
	if err := manifest.withImplicitTargets(files, "pkg"); err != nil {
		t.Fatal(err)
	}
	if len(manifest.targets) != 3 || manifest.targets[0].path != "src/lib.rs" || manifest.targets[1].path != "src/main.rs" || manifest.targets[2].path != "tests/smoke.rs" {
		t.Errorf("targets = %+v", manifest.targets)
	}
}

func TestCargoImplicitTargetFailures(t *testing.T) {
	manifest := &cargoManifest{targets: []cargoTarget{
		{kind: libraryKind, name: "same", path: "src/lib.rs"},
		{kind: binaryKind, name: "same", path: "src/main.rs"},
	}}
	if err := manifest.validateTargetClaims(); err == nil {
		t.Error("duplicate target name accepted")
	}
	manifest.targets[1].name = "other"
	manifest.targets[1].path = "src/lib.rs"
	if err := manifest.validateTargetClaims(); err == nil {
		t.Error("duplicate target path accepted")
	}
	manifest = &cargoManifest{packageName: "app"}
	if err := manifest.withImplicitTargets(map[string]bool{"pkg/tests/---.rs": true}, "pkg"); err == nil {
		t.Error("invalid implicit test name accepted")
	}
	manifest = &cargoManifest{packageName: "---"}
	if err := manifest.withImplicitTargets(map[string]bool{"pkg/src/lib.rs": true}, "pkg"); err == nil {
		t.Error("invalid implicit package name accepted")
	}
	manifest = &cargoManifest{packageName: "app"}
	if err := manifest.withImplicitTargets(map[string]bool{"pkg/src/lib.rs": true}, "pkg"); err != nil || len(manifest.targets) != 1 {
		t.Errorf("direct implicit library = %+v, %v", manifest.targets, err)
	}
}

func TestCargoLibBinDisambiguation(t *testing.T) {
	manifest, err := parseCargoManifest("Cargo.toml", []byte("[package]\nname = \"demo\"\n[lib]\nname = \"demo\"\npath = \"src/lib.rs\"\n[[bin]]\nname = \"demo\"\npath = \"src/main.rs\"\n"))
	if err != nil {
		t.Fatal(err)
	}
	if len(manifest.targets) != 2 || manifest.targets[0].name != "demo_lib" || manifest.targets[0].crate() != "demo" || manifest.targets[1].name != "demo" {
		t.Errorf("explicit lib/bin targets = %+v", manifest.targets)
	}
	implicit := &cargoManifest{packageName: "app"}
	files := map[string]bool{"pkg/src/lib.rs": true, "pkg/src/main.rs": true}
	if err := implicit.withImplicitTargets(files, "pkg"); err != nil {
		t.Fatal(err)
	}
	if len(implicit.targets) != 2 || implicit.targets[0].name != "app_lib" || implicit.targets[0].crate() != "app" || implicit.targets[1].name != "app" {
		t.Errorf("implicit lib/bin targets = %+v", implicit.targets)
	}
	solo, err := parseCargoManifest("Cargo.toml", []byte("[package]\nname = \"solo\"\n[lib]\nname = \"solo\"\npath = \"src/lib.rs\"\n"))
	if err != nil {
		t.Fatal(err)
	}
	if len(solo.targets) != 1 || solo.targets[0].name != "solo" || solo.targets[0].crate() != "solo" {
		t.Errorf("solo library targets = %+v", solo.targets)
	}
}

func TestCargoBinOnlyHasNoPhantomLib(t *testing.T) {
	manifest, err := parseCargoManifest("Cargo.toml", []byte("[package]\nname = \"worker\"\n[[bin]]\nname = \"worker\"\npath = \"src/main.rs\"\n[dependencies]\napi = { path = \"../api\" }\n"))
	if err != nil {
		t.Fatal(err)
	}
	if len(manifest.targets) != 1 || manifest.targets[0].kind != binaryKind || manifest.targets[0].name != "worker" {
		t.Fatalf("bin-only targets = %+v", manifest.targets)
	}
	if err := manifest.withImplicitTargets(map[string]bool{"crates/worker/src/main.rs": true}, "crates/worker"); err != nil {
		t.Fatal(err)
	}
	if len(manifest.targets) != 1 || manifest.targets[0].kind != binaryKind {
		t.Fatalf("bin-only implicit targets = %+v", manifest.targets)
	}
	if got := siblingLibName(manifest, manifest.targets[0]); got != "" {
		t.Errorf("siblingLibName(bin-only) = %q, want empty", got)
	}
}

func TestSiblingLibName(t *testing.T) {
	withLib := &cargoManifest{targets: []cargoTarget{{kind: libraryKind, name: "demo_lib"}, {kind: binaryKind, name: "demo"}}}
	if got := siblingLibName(withLib, cargoTarget{kind: binaryKind, name: "demo"}); got != "demo_lib" {
		t.Errorf("siblingLibName with library = %q, want %q", got, "demo_lib")
	}
	binOnly := &cargoManifest{targets: []cargoTarget{{kind: binaryKind, name: "tool"}}}
	if got := siblingLibName(binOnly, cargoTarget{kind: binaryKind, name: "tool"}); got != "" {
		t.Errorf("siblingLibName without library = %q, want empty", got)
	}
}

func TestCargoCallNamesDefensive(t *testing.T) {
	if got := cargoCallNames(&bzl.StringExpr{Value: "deps"}); got != nil {
		t.Errorf("cargoCallNames(string) = %v, want nil", got)
	}
	if got := cargoCallNames(&bzl.CallExpr{X: &bzl.Ident{Name: "deps"}, List: []bzl.Expr{strListExpr("a")}}); got != nil {
		t.Errorf("cargoCallNames(deps(...)) = %v, want nil", got)
	}
	if got := cargoCallNames(&bzl.CallExpr{X: &bzl.Ident{Name: "crate_deps"}}); got != nil {
		t.Errorf("cargoCallNames(crate_deps()) = %v, want nil", got)
	}
	if got := cargoCallNames(&bzl.CallExpr{X: &bzl.Ident{Name: "crate_deps"}, List: []bzl.Expr{&bzl.StringExpr{Value: "x"}}}); got != nil {
		t.Errorf("cargoCallNames(crate_deps(string)) = %v, want nil", got)
	}
	if got := cargoCallNames(crateDepsFileExpr([]string{"serde_json"}, "pkg")); len(got) != 1 || got[0] != "serde_json" {
		t.Errorf("cargoCallNames(crate_deps([...])) = %v, want [serde_json]", got)
	}
}

func TestParseCargoExamplesBenches(t *testing.T) {
	manifest, err := parseCargoManifest("Cargo.toml", []byte(`[package]
name = "shapes"
version = "0.3.0"

[lib]
name = "shapes_lib"
path = "source/lib.rs"

[[example]]
name = "demo"
path = "examples/demo.rs"
test = true

[[example]]
name = "plain"
path = "examples/plain.rs"

[[bench]]
name = "criterion"
path = "benches/criterion.rs"
harness = false
`))
	if err != nil {
		t.Fatal(err)
	}
	if manifest.version != "0.3.0" {
		t.Errorf("package version = %q, want 0.3.0", manifest.version)
	}
	if len(manifest.targets) != 4 {
		t.Fatalf("targets = %+v, want 4", manifest.targets)
	}
	demo := manifest.targets[1]
	if demo.kind != exampleKind || demo.name != "demo_example" || demo.logical != "demo" || !demo.exampleTest {
		t.Errorf("demo example = %+v", demo)
	}
	plain := manifest.targets[2]
	if plain.kind != exampleKind || plain.name != "plain_example" || plain.exampleTest {
		t.Errorf("plain example = %+v", plain)
	}
	bench := manifest.targets[3]
	if bench.kind != benchKind || bench.name != "criterion_bench" || bench.logical != "criterion" || bench.harness {
		t.Errorf("criterion bench = %+v", bench)
	}
}

func TestParseCargoExampleBenchFailures(t *testing.T) {
	cases := []struct {
		name   string
		source string
		want   string
	}{
		{
			name:   "bench harness enabled",
			source: "[package]\nname = \"demo\"\n[[bench]]\nname = \"b\"\nharness = true\n",
			want:   "harness",
		},
		{
			name:   "bench harness default",
			source: "[package]\nname = \"demo\"\n[[bench]]\nname = \"b\"\n",
			want:   "harness",
		},
		{
			name:   "required features",
			source: "[package]\nname = \"demo\"\n[[example]]\nname = \"e\"\nrequired-features = [\"fast\"]\n",
			want:   "required-features",
		},
		{
			name:   "example harness rejected",
			source: "[package]\nname = \"demo\"\n[[example]]\nname = \"e\"\nharness = false\n",
			want:   "harness",
		},
	}
	for _, tc := range cases {
		if _, err := parseCargoManifest("Cargo.toml", []byte(tc.source)); err == nil || !strings.Contains(err.Error(), tc.want) {
			t.Errorf("%s: err = %v, want %q", tc.name, err, tc.want)
		}
	}
}

func TestParseCargoLibFlavors(t *testing.T) {
	cases := []struct {
		source string
		flavor string
	}{
		{"[package]\nname = \"demo\"\n[lib]\nproc-macro = true\n", "proc-macro"},
		{"[package]\nname = \"demo\"\n[lib]\ncrate-type = [\"cdylib\"]\n", "cdylib"},
		{"[package]\nname = \"demo\"\n[lib]\ncrate-type = [\"staticlib\"]\n", "staticlib"},
		{"[package]\nname = \"demo\"\n[lib]\ncrate-type = [\"rlib\"]\n", ""},
		{"[package]\nname = \"demo\"\n[lib]\ncrate-type = [\"proc-macro\"]\n", "proc-macro"},
		{"[package]\nname = \"demo\"\n[lib]\ncrate-type = []\n", ""},
		{"[package]\nname = \"demo\"\n[lib]\n", ""},
	}
	for _, tc := range cases {
		manifest, err := parseCargoManifest("Cargo.toml", []byte(tc.source))
		if err != nil {
			t.Errorf("source %q: unexpected error %v", tc.source, err)
			continue
		}
		if manifest.targets[0].flavor != tc.flavor {
			t.Errorf("source %q: flavor = %q, want %q", tc.source, manifest.targets[0].flavor, tc.flavor)
		}
	}
	failures := []string{
		"[package]\nname = \"demo\"\n[lib]\ncrate-type = [\"cdylib\", \"staticlib\"]\n",
		"[package]\nname = \"demo\"\n[lib]\ncrate-type = [\"dylib\"]\n",
		"[package]\nname = \"demo\"\n[lib]\nproc-macro = true\ncrate-type = [\"cdylib\"]\n",
		"[package]\nname = \"demo\"\n[lib]\nproc-macro = true\ncrate-type = [\"staticlib\"]\n",
		"[package]\nname = \"demo\"\n[lib]\nproc-macro = true\ncrate-type = [\"rlib\"]\n",
	}
	for _, source := range failures {
		if _, err := parseCargoManifest("Cargo.toml", []byte(source)); err == nil {
			t.Errorf("expected failure for %q", source)
		}
	}
}

func TestParseCargoBuildScript(t *testing.T) {
	disabled, err := parseCargoManifest("Cargo.toml", []byte("[package]\nname = \"demo\"\nbuild = false\n"))
	if err != nil {
		t.Fatal(err)
	}
	if disabled.build == nil || !disabled.build.disabled {
		t.Errorf("build = false: %+v, want disabled", disabled.build)
	}
	explicit, err := parseCargoManifest("Cargo.toml", []byte("[package]\nname = \"demo\"\nbuild = \"tool/gen.rs\"\n"))
	if err != nil {
		t.Fatal(err)
	}
	if explicit.build == nil || explicit.build.disabled || explicit.build.path != "tool/gen.rs" {
		t.Errorf("build path: %+v", explicit.build)
	}
	def, err := parseCargoManifest("Cargo.toml", []byte("[package]\nname = \"demo\"\nbuild = true\n"))
	if err != nil {
		t.Fatal(err)
	}
	if def.build == nil || def.build.path != "build.rs" {
		t.Errorf("build = true: %+v, want build.rs", def.build)
	}
	undeclared, err := parseCargoManifest("Cargo.toml", []byte("[package]\nname = \"demo\"\n"))
	if err != nil {
		t.Fatal(err)
	}
	if err := undeclared.withImplicitTargets(map[string]bool{"build.rs": true}, ""); err == nil || !strings.Contains(err.Error(), "build.rs") {
		t.Errorf("undeclared build.rs err = %v", err)
	}
	scoped, err := parseCargoManifest("Cargo.toml", []byte("[package]\nname = \"demo\"\n[build-dependencies]\ncc = \"1\"\nlocal = { path = \"../local\", version = \"0.2\" }\n"))
	if err != nil {
		t.Fatal(err)
	}
	if !scoped.buildDeps["cc"].external {
		t.Errorf("cc build dep = %+v, want external", scoped.buildDeps["cc"])
	}
	if local := scoped.buildDeps["local"]; local.external || local.version != "0.2" || local.depPath != "../local" {
		t.Errorf("local build dep = %+v", local)
	}
}

func TestVersionReqSatisfied(t *testing.T) {
	cases := []struct {
		version string
		req     string
		want    bool
	}{
		{"1.2.3", "1", true},
		{"1.9.0", "^1.2", true},
		{"2.0.0", "1", false},
		{"0.2.5", "0.2", true},
		{"0.3.0", "0.2", false},
		{"0.5.0", "^0", true},
		{"1.0.0", "^0", false},
		{"0.0.3", "^0.0.3", true},
		{"0.0.4", "^0.0.3", false},
		{"1.2.3", "~1.2.0", true},
		{"1.3.0", "~1.2.0", false},
		{"1.9.0", "~1", true},
		{"2.0.0", "~1", false},
		{"1.2.3", "=1.2.3", true},
		{"1.2.4", "=1.2.3", false},
		{"1.2.3", ">=1.0, <2.0", true},
		{"2.0.0", ">=1.0, <2.0", false},
		{"1.2.3", "<=1.2.3", true},
		{"1.2.4", "<=1.2.3", false},
		{"1.2.4", ">1.2.3", true},
		{"1.2.3", ">1.2.3", false},
		{"1.2.3-rc.1", "^1.0", true},
		{"1.2.3+build", "=1.2.3", true},
		{"1.2.3.4", "=1.2.3", true},
		{"abc", ">=1.0", false},
		{"1.2.3", "*", true},
		{"1.2.3", "", true},
	}
	for _, tc := range cases {
		if got := versionReqSatisfied(tc.version, tc.req); got != tc.want {
			t.Errorf("versionReqSatisfied(%q, %q) = %v, want %v", tc.version, tc.req, got, tc.want)
		}
	}
	inline, err := tomlInlineTable(`{ myversion = "9", 'version' = "1.2", features = ["a", "b"], "x.y" = { z = 1 }, a.b = 'c' }`)
	if err != nil {
		t.Fatal(err)
	}
	if inline["version"] != `"1.2"` || inline["myversion"] != `"9"` || inline["features"] != `["a", "b"]` || inline["x.y"] != "{ z = 1 }" || inline["a.b"] != "'c'" {
		t.Errorf("inline table = %v", inline)
	}
	if empty, err := tomlInlineTable("{ }"); err != nil || len(empty) != 0 {
		t.Errorf("empty inline table = %v, %v", empty, err)
	}
	for _, bad := range []string{`"1"`, `{ version = "abc }`, `{ v = 'abc }`, `{ features = ["a" }`, `{ version }`, `{ version = }`, `{ "a = 1 }`, `{ 'a = 1 }`, `{ .a = 1 }`} {
		if _, err := tomlInlineTable(bad); err == nil {
			t.Errorf("tomlInlineTable(%s) accepted", bad)
		}
	}
	if got := scanPackageVersion([]byte("[package]\nname = \"demo\"\nversion = \"0.2.0\"\n")); got != "0.2.0" {
		t.Errorf("scanned version = %q, want 0.2.0", got)
	}
	if got := scanPackageVersion([]byte("[package]\n\nversion = \"1.0\"\n")); got != "1.0" {
		t.Errorf("scanned version past blank = %q, want 1.0", got)
	}
	if got := scanPackageVersion([]byte("[dependencies]\nfoo = \"1\"\n[package]\nversion = \"1.0\"\n")); got != "1.0" {
		t.Errorf("scanned version past section = %q, want 1.0", got)
	}
	if got := scanPackageVersion([]byte("[package]\nname = \"noversion\"\n")); got != "" {
		t.Errorf("absent version = %q, want empty", got)
	}
}

func TestCheckPathDepVersions(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "helper/Cargo.toml", "[package]\nname = \"helper\"\nversion = \"0.2.0\"\n")
	writeFixture(t, root, "noversion/Cargo.toml", "[package]\nname = \"noversion\"\n")
	manifest := &cargoManifest{
		normalDeps: map[string]cargoDependency{
			"helper": {version: "^0.2", depPath: "helper"},
			"free":   {depPath: "helper"},
			"ext":    {external: true, version: "1", depPath: "helper"},
		},
		devDeps:   map[string]cargoDependency{},
		buildDeps: map[string]cargoDependency{},
	}
	if err := checkPathDepVersions(root, "Cargo.toml", manifest); err != nil {
		t.Errorf("satisfied path versions rejected: %v", err)
	}
	manifest.normalDeps["helper"] = cargoDependency{version: "^0.3", depPath: "helper"}
	if err := checkPathDepVersions(root, "Cargo.toml", manifest); err == nil || !strings.Contains(err.Error(), "does not satisfy") {
		t.Errorf("mismatched path version err = %v", err)
	}
	manifest.normalDeps["helper"] = cargoDependency{version: "1", depPath: "noversion"}
	if err := checkPathDepVersions(root, "Cargo.toml", manifest); err == nil || !strings.Contains(err.Error(), "declares no [package] version") {
		t.Errorf("unversioned provider err = %v", err)
	}
	manifest.normalDeps["helper"] = cargoDependency{version: "1", depPath: "missing"}
	if err := checkPathDepVersions(root, "Cargo.toml", manifest); err == nil || !strings.Contains(err.Error(), "without a readable Cargo.toml") {
		t.Errorf("missing provider err = %v", err)
	}
}

func TestParseCargoExamplePathDerived(t *testing.T) {
	manifest, err := parseCargoManifest("Cargo.toml", []byte("[package]\nname = \"demo\"\n[[example]]\npath = \"examples/foo.rs\"\n"))
	if err != nil {
		t.Fatal(err)
	}
	if len(manifest.targets) != 1 {
		t.Fatalf("targets = %+v", manifest.targets)
	}
	example := manifest.targets[0]
	if example.kind != exampleKind || example.name != "foo_example" || example.logical != "foo" || example.path != "examples/foo.rs" {
		t.Errorf("path-derived example = %+v", example)
	}
	if _, err := parseCargoManifest("Cargo.toml", []byte("[package]\nname = \"demo\"\n[dependencies]\nfoo = { version = \"1\", path\n")); err == nil || !strings.Contains(err.Error(), "Cargo.toml:4: dependency \"foo\"") {
		t.Errorf("truncated inline dep err = %v", err)
	}
}

func TestParseCargoVirtualWorkspace(t *testing.T) {
	virtual, err := parseCargoManifest("Cargo.toml", []byte("[workspace]\nmembers = [\"crates/api\"]\nresolver = \"2\"\n"))
	if err != nil {
		t.Fatal(err)
	}
	if !virtual.virtual || len(virtual.targets) != 0 {
		t.Errorf("virtual manifest = %+v, want virtual with no targets", virtual)
	}
	dotted, err := parseCargoManifest("Cargo.toml", []byte("[workspace.dependencies]\nserde = \"1\"\n"))
	if err != nil {
		t.Fatal(err)
	}
	if !dotted.virtual {
		t.Errorf("dotted workspace manifest = %+v, want virtual", dotted)
	}
	packaged, err := parseCargoManifest("Cargo.toml", []byte("[workspace]\n\n[package]\nname = \"demo\"\n"))
	if err != nil {
		t.Fatal(err)
	}
	if packaged.virtual || packaged.packageName != "demo" {
		t.Errorf("root package manifest = %+v, want non-virtual demo", packaged)
	}
	if _, err := parseCargoManifest("Cargo.toml", []byte("readme = true\n")); err == nil {
		t.Error("bare manifest without [package] or [workspace] accepted")
	}
}

func TestCargoDependencyForms(t *testing.T) {
	const pkg = "[package]\nname = \"demo\"\n"
	type want struct {
		table     string
		key       string
		label     string
		external  bool
		depPath   string
		version   string
		workspace bool
	}
	cases := []struct {
		name     string
		manifest string
		want     []want
	}{
		{"dotted workspace", pkg + "[dependencies]\nserde.workspace = true\n", []want{{"normal", "serde", "serde", true, "", "", true}}},
		{"inline workspace", pkg + "[dependencies]\nserde = { workspace = true, features = [\"derive\"] }\n", []want{{"normal", "serde", "serde", true, "", "", true}}},
		{"table version", pkg + "[dependencies.serde]\nversion = \"1\"\nfeatures = [\"derive\"]\n", []want{{"normal", "serde", "serde", true, "", "1", false}}},
		{"table path", pkg + "[dev-dependencies.helper-lib]\npath = \"../helper\"\nversion = \"0.2\"\n", []want{{"dev", "helper_lib", "helper-lib", false, "../helper", "0.2", false}}},
		{"target cfg", pkg + "[target.'cfg(unix)'.dependencies]\nlibc = \"0.2\"\n", []want{{"normal", "libc", "libc", true, "", "0.2", false}}},
		{"target triple dev", pkg + "[target.\"x86_64-pc-windows-msvc\".dev-dependencies]\nwinapi = \"0.3\"\n", []want{{"dev", "winapi", "winapi", true, "", "0.3", false}}},
		{"target cfg table", pkg + "[target.'cfg(any(unix, target_os = \"wasi\"))'.build-dependencies.nix]\nversion = \"0.27\"\n", []want{{"build", "nix", "nix", true, "", "0.27", false}}},
		{"xpath feature", pkg + "[dependencies]\nfoo = { version = \"1\", features = [\"xpath\"] }\n", []want{{"normal", "foo", "foo", true, "", "1", false}}},
		{"package rename", pkg + "[dependencies]\nfoo = { package = \"pathdiff\", version = \"0.2\" }\n", []want{{"normal", "foo", "pathdiff", true, "", "0.2", false}}},
		{"local path", pkg + "[dependencies]\nx = { path = \"../x\" }\n", []want{{"normal", "x", "x", false, "../x", "", false}}},
		{"workspace deps", "[workspace]\nmembers = [\"a\"]\n[workspace.dependencies]\nserde = \"1\"\napi = { path = \"crates/api\", version = \"0.1\" }\n[workspace.dependencies.json]\npackage = \"serde_json\"\n", []want{
			{"workspace", "serde", "serde", true, "", "1", false},
			{"workspace", "api", "api", false, "crates/api", "0.1", false},
			{"workspace", "json", "serde_json", true, "", "", false},
		}},
		{"quoted key", pkg + "[dependencies]\n\"serde-json\" = \"1\"\n'tokio'.version = \"1\"\n", []want{
			{"normal", "serde_json", "serde-json", true, "", "1", false},
			{"normal", "tokio", "tokio", true, "", "1", false},
		}},
		{"unconditional wins", pkg + "[target.'cfg(unix)'.dependencies]\nlibc = { path = \"../libc\" }\n[dependencies]\nlibc = \"0.2\"\n[target.'cfg(windows)'.dependencies]\nlibc = { path = \"../win\" }\n", []want{{"normal", "libc", "libc", true, "", "0.2", false}}},
		{"workspace false", pkg + "[dependencies]\nfoo = { version = \"1\", workspace = false, package = \"bar\" }\n", []want{{"normal", "foo", "bar", true, "", "1", false}}},
		{"comment quote", pkg + "description = \"say \\\"hi\\\" # [\"\n[dependencies]\nfoo = \"1\" # it's fine\n[target.'cfg(feature = \"a#b\")'.dependencies]\nbar = \"2\"\n", []want{
			{"normal", "foo", "foo", true, "", "1", false},
			{"normal", "bar", "bar", true, "", "2", false},
		}},
		{"other headers", pkg + "[dependencies.serde.extra]\nx = 1\n[workspace.dependencies.a.b]\ny = 2\n[target.unix]\nz = 3\n[target.'cfg(unix)'.other]\nw = 4\n", nil},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			manifest, err := parseCargoManifest("Cargo.toml", []byte(tc.manifest))
			if err != nil {
				t.Fatal(err)
			}
			tables := map[string]map[string]cargoDependency{"normal": manifest.normalDeps, "dev": manifest.devDeps, "build": manifest.buildDeps, "workspace": manifest.workspaceDeps}
			count := 0
			for _, deps := range tables {
				count += len(deps)
			}
			if count != len(tc.want) {
				t.Errorf("deps = %+v, want %d entries", tables, len(tc.want))
			}
			for _, w := range tc.want {
				got, ok := tables[w.table][w.key]
				expected := cargoDependency{external: w.external, label: w.label, version: w.version, depPath: w.depPath, workspace: w.workspace}
				if !ok || got != expected {
					t.Errorf("%s[%s] = %+v (present %v), want %+v", w.table, w.key, got, ok, expected)
				}
			}
		})
	}
	failures := []struct {
		manifest string
		want     string
	}{
		{pkg + "[target.'cfg(unix).dependencies]\n", "Cargo.toml:3: malformed Cargo table header"},
		{pkg + "[dependencies]\n'foo.version = \"1\"\n", "Cargo.toml:4: malformed dependency key"},
		{pkg + "[dependencies]\nfoo = 1\n", "Cargo.toml:4: dependency \"foo\" must be a version string or a table"},
		{pkg + "[dependencies]\nfoo = { version = }\n", "Cargo.toml:4: dependency \"foo\": inline table key \"version\" has no value"},
		{pkg + "[dependencies]\nfoo = { version = 1 }\n", "Cargo.toml:4: dependency \"foo\" version: expected a basic quoted string"},
		{pkg + "[dependencies]\nfoo = { path = 1 }\n", "Cargo.toml:4: dependency \"foo\" path: expected a basic quoted string"},
		{pkg + "[dependencies.foo]\npackage = true\n", "Cargo.toml:4: dependency \"foo\" package: expected a basic quoted string"},
		{pkg + "[dependencies]\nfoo.workspace = \"yes\"\n", "Cargo.toml:4: dependency \"foo\" workspace must be true or false"},
		{pkg + "[dependencies]\nfoo.workspace = true\nfoo.package = \"bar\"\n", "Cargo.toml:5: dependency \"foo\" sets both workspace = true and package"},
	}
	for _, tc := range failures {
		if _, err := parseCargoManifest("Cargo.toml", []byte(tc.manifest)); err == nil || !strings.Contains(err.Error(), tc.want) {
			t.Errorf("parse %q err = %v, want %q", tc.manifest, err, tc.want)
		}
	}
	root, err := parseCargoManifest("Cargo.toml", []byte("[workspace]\n"))
	if err != nil || !root.workspaceRoot {
		t.Errorf("workspace root = %+v, %v", root, err)
	}
	member, err := parseCargoManifest("Cargo.toml", []byte(pkg))
	if err != nil || member.workspaceRoot {
		t.Errorf("member = %+v, %v", member, err)
	}
	if parts, err := splitTomlDottedKey(` target . "a.\"b" . 'c.d' `); err != nil || strings.Join(parts, "|") != `target|a."b|c.d` {
		t.Errorf("split dotted key = %q, %v", parts, err)
	}
	for _, bad := range []string{`a..b`, `"a\q"`, `'a' x`, `"a`} {
		if _, err := splitTomlDottedKey(bad); err == nil {
			t.Errorf("splitTomlDottedKey(%s) accepted", bad)
		}
	}
	if got := stripTomlComment(`[target.'cfg(feature = "a#b")'.dependencies] # c`); got != `[target.'cfg(feature = "a#b")'.dependencies] ` {
		t.Errorf("literal-string comment strip = %q", got)
	}
}

func TestCargoInheritWorkspaceDeps(t *testing.T) {
	root, err := parseCargoManifest("Cargo.toml", []byte("[workspace]\n[workspace.dependencies]\nserde = \"1\"\njson = { package = \"serde_json\", version = \"1.0\" }\napi = { path = \"crates/api\", version = \"0.1.0\" }\n"))
	if err != nil {
		t.Fatal(err)
	}
	member, err := parseCargoManifest("crates/app/Cargo.toml", []byte("[package]\nname = \"app\"\n[dependencies]\nserde.workspace = true\njson = { workspace = true, features = [\"std\"] }\nlocal = { path = \"../local\" }\n[dev-dependencies]\napi.workspace = true\n"))
	if err != nil {
		t.Fatal(err)
	}
	if err := member.inheritWorkspaceDeps("crates/app", "", root); err != nil {
		t.Fatal(err)
	}
	wants := map[string]cargoDependency{
		"serde": {external: true, label: "serde", version: "1", workspace: true},
		"json":  {external: true, label: "serde_json", version: "1.0", workspace: true},
		"local": {label: "local", depPath: "../local"},
	}
	for key, want := range wants {
		if got := member.normalDeps[key]; got != want {
			t.Errorf("normal %s = %+v, want %+v", key, got, want)
		}
	}
	if got, want := member.devDeps["api"], (cargoDependency{label: "api", version: "0.1.0", depPath: "../api", workspace: true}); got != want {
		t.Errorf("dev api = %+v, want %+v", got, want)
	}
	nested, err := parseCargoManifest("tools/x/Cargo.toml", []byte("[package]\nname = \"x\"\n[build-dependencies]\napi.workspace = true\n"))
	if err != nil {
		t.Fatal(err)
	}
	if err := nested.inheritWorkspaceDeps("tools/x", "", root); err != nil || nested.buildDeps["api"].depPath != "../../crates/api" {
		t.Errorf("nested api = %+v, %v", nested.buildDeps["api"], err)
	}
	self, err := parseCargoManifest("Cargo.toml", []byte("[package]\nname = \"x\"\n[dependencies]\napi.workspace = true\n"))
	if err != nil {
		t.Fatal(err)
	}
	if err := self.inheritWorkspaceDeps("crates/api", "crates/api", &cargoManifest{workspaceDeps: map[string]cargoDependency{"api": {label: "api"}}}); err != nil || self.normalDeps["api"].depPath != "." {
		t.Errorf("self api = %+v, %v", self.normalDeps["api"], err)
	}
	if got := relSlashPath("", "crates/api"); got != "crates/api" {
		t.Errorf("relSlashPath from root = %q", got)
	}
	missing, err := parseCargoManifest("crates/app/Cargo.toml", []byte("[package]\nname = \"app\"\n[dependencies]\nrand-core.workspace = true\n"))
	if err != nil {
		t.Fatal(err)
	}
	if name, ok := missing.workspaceDep(); !ok || name != "rand-core" {
		t.Errorf("workspaceDep = %q, %v", name, ok)
	}
	if _, ok := member.workspaceDep(); !ok {
		t.Error("member workspace dep not reported")
	}
	if _, ok := root.workspaceDep(); ok {
		t.Error("root reports a workspace dep")
	}
	if err := missing.inheritWorkspaceDeps("crates/app", "nested", root); err == nil || err.Error() != `dependency "rand-core" sets workspace = true but nested/Cargo.toml has no [workspace.dependencies] entry` {
		t.Errorf("missing entry err = %v", err)
	}
}
