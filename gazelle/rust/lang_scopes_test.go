package rust

import (
	"strings"
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	bzl "github.com/bazelbuild/buildtools/build"
	"github.com/ralvik/rules_dx/gazelle/common"
)

func TestResolveOverrideAndConflict(t *testing.T) {
	l := &rustLang{}
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "rust rust mapped //mapped:dep"}})
	cfg.Exts[languageName] = &rustConfig{}
	r := rule.NewRule(binaryKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, targetImports{production: []string{"mapped"}}, label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//mapped:dep" {
		t.Errorf("override deps = %q", got)
	}
	ignore := &common.IgnoreEntry{Value: "mapped"}
	cfg.Exts[languageName] = &rustConfig{IgnoreConfig: common.IgnoreConfig{Ignores: []*common.IgnoreEntry{ignore}}}
	l.Resolve(cfg, resolverIndex(l), nil, rule.NewRule(binaryKind, "conflict"), targetImports{production: []string{"mapped"}}, label.New("", "app", "conflict"))
	if !ignore.Used || len(l.errors) != 1 || !strings.Contains(l.errors[0], "both") {
		t.Errorf("mapping-ignore conflict = used:%v errors:%v", ignore.Used, l.errors)
	}
}

func TestResolveAmbiguous(t *testing.T) {
	l := &rustLang{}
	cfg := resolverConfig(t, nil)
	cfg.Exts[languageName] = &rustConfig{}
	index := resolverIndex(l,
		struct{ pkg, name string }{"one", "same"},
		struct{ pkg, name string }{"two", "same"},
	)
	l.Resolve(cfg, index, nil, rule.NewRule(binaryKind, "app"), targetImports{production: []string{"same"}}, label.New("", "app", "app"))
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "ambiguous") || !strings.Contains(l.errors[0], "//one:same") || !strings.Contains(l.errors[0], "//two:same") {
		t.Errorf("ambiguity = %v", l.errors)
	}
}

func TestLookupOverrideNilConfig(t *testing.T) {
	if _, ok := lookupOverride(nil, "anything"); ok {
		t.Error("lookupOverride(nil, ...) = ok, want not ok")
	}
}

func TestResolveScriptDepEdge(t *testing.T) {
	l := &rustLang{}
	cfg := resolverConfig(t, nil)
	cfg.Exts[languageName] = &rustConfig{}
	r := rule.NewRule(libraryKind, "scripted_lib")
	l.Resolve(cfg, resolverIndex(l), nil, r, targetImports{scriptDep: "scripted_build_script"}, label.New("", "scripted", "scripted_lib"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != ":scripted_build_script" {
		t.Errorf("script-dep deps = %q", got)
	}
	if len(l.errors) != 0 {
		t.Errorf("script-dep errors = %v", l.errors)
	}
	self := rule.NewRule(scriptKind, "scripted_build_script")
	l.Resolve(cfg, resolverIndex(l), nil, self, targetImports{scriptDep: "scripted_build_script"}, label.New("", "scripted", "scripted_build_script"))
	if self.Attr("deps") != nil {
		t.Errorf("self script-dep deps = %v, want none", self.Attr("deps"))
	}
}

func TestValidateTestImportMapping(t *testing.T) {
	manifest := &cargoManifest{
		packageName: "demo",
		normalDeps:  map[string]cargoDependency{},
		devDeps:     map[string]cargoDependency{},
	}
	c := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "rust tmapped //pkg:target"}})
	mapped := targetImports{test: []string{"tmapped"}}
	if err := validateCargoImports(c, manifest, testKind, mapped); err != nil {
		t.Errorf("override-only test import rejected: %v", err)
	}
	c.Exts[languageName] = &rustConfig{IgnoreConfig: common.IgnoreConfig{Ignores: []*common.IgnoreEntry{{Value: "tmapped"}}}}
	if err := validateCargoImports(c, manifest, testKind, mapped); err == nil || !strings.Contains(err.Error(), "both") {
		t.Errorf("test mapping/ignore conflict not reported: %v", err)
	}
}

func TestLocalTestImportResolveOverride(t *testing.T) {
	manifest := &cargoManifest{
		packageName: "demo",
		normalDeps:  map[string]cargoDependency{},
		devDeps:     map[string]cargoDependency{},
	}
	c := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "rust tmapped //pkg:target"}})
	local := localCargoImports(c, manifest, targetImports{test: []string{"tmapped"}}, true)
	if strings.Join(local.test, ",") != "tmapped" {
		t.Errorf("override-only local test imports = %+v, want [tmapped]", local.test)
	}
}

func TestDxKindForFlavors(t *testing.T) {
	cases := []struct {
		target cargoTarget
		want   string
	}{
		{cargoTarget{kind: libraryKind}, libraryKind},
		{cargoTarget{kind: libraryKind, flavor: "proc-macro"}, procMacroKind},
		{cargoTarget{kind: libraryKind, flavor: "cdylib"}, sharedKind},
		{cargoTarget{kind: libraryKind, flavor: "staticlib"}, staticKind},
		{cargoTarget{kind: binaryKind}, binaryKind},
		{cargoTarget{kind: testKind}, testKind},
		{cargoTarget{kind: exampleKind}, binaryKind},
		{cargoTarget{kind: benchKind}, binaryKind},
	}
	for _, tc := range cases {
		if got := dxKindFor(tc.target); got != tc.want {
			t.Errorf("dxKindFor(%+v) = %q, want %q", tc.target, got, tc.want)
		}
	}
}

func TestWantsUnitTestKinds(t *testing.T) {
	tested := map[string]*FileFacts{"src/lib.rs": {HasTestAttr: true}}
	plain := map[string]*FileFacts{"src/lib.rs": {}}
	cases := []struct {
		name   string
		target cargoTarget
		tree   map[string]*FileFacts
		want   bool
	}{
		{"lib tested", cargoTarget{kind: libraryKind}, tested, true},
		{"lib plain", cargoTarget{kind: libraryKind}, plain, false},
		{"bin tested", cargoTarget{kind: binaryKind}, tested, true},
		{"test never", cargoTarget{kind: testKind}, tested, false},
		{"bench never", cargoTarget{kind: benchKind}, tested, false},
		{"example test=true", cargoTarget{kind: exampleKind, exampleTest: true}, tested, true},
		{"example test=false", cargoTarget{kind: exampleKind}, tested, false},
	}
	for _, tc := range cases {
		if got := wantsUnitTest(tc.target, tc.tree); got != tc.want {
			t.Errorf("%s: wantsUnitTest = %v, want %v", tc.name, got, tc.want)
		}
	}
}

func TestExampleBuildImportScopes(t *testing.T) {
	manifest := &cargoManifest{
		packageName: "demo",
		normalDeps:  map[string]cargoDependency{"serde_json": {external: true}},
		devDeps:     map[string]cargoDependency{"tempfile": {external: true}},
		buildDeps:   map[string]cargoDependency{"cc": {external: true}},
	}
	c := resolverConfig(t, nil)
	if err := validateExampleImports(c, manifest, targetImports{production: []string{"serde_json", "tempfile"}}); err != nil {
		t.Errorf("dev import in example rejected: %v", err)
	}
	if err := validateExampleImports(c, manifest, targetImports{production: []string{"missing"}}); err == nil {
		t.Error("undeclared example import accepted")
	}
	if err := validateExampleImports(c, manifest, targetImports{test: []string{"tempfile"}}); err != nil {
		t.Errorf("dev test import in example rejected: %v", err)
	}
	if err := validateExampleImports(c, manifest, targetImports{test: []string{"missing"}}); err == nil || !strings.Contains(err.Error(), "unresolved test import") {
		t.Errorf("undeclared example test import err = %v", err)
	}
	if err := validateBuildImports(c, manifest, targetImports{production: []string{"cc"}}); err != nil {
		t.Errorf("build-dep import rejected: %v", err)
	}
	if err := validateBuildImports(c, manifest, targetImports{production: []string{"serde_json"}}); err == nil {
		t.Error("normal import in build script accepted")
	}
	if err := validateBuildImports(c, manifest, targetImports{test: []string{"cc"}}); err != nil {
		t.Errorf("build-dep test import rejected: %v", err)
	}
	if err := validateBuildImports(c, manifest, targetImports{test: []string{"tempfile"}}); err == nil {
		t.Error("dev test import in build script accepted")
	}
	scoped := &cargoManifest{
		packageName: "demo",
		normalDeps:  map[string]cargoDependency{"local": {}},
		devDeps:     map[string]cargoDependency{"devlocal": {}},
		buildDeps:   map[string]cargoDependency{"buildlocal": {}},
	}
	if got := localCargoExampleImports(c, scoped, targetImports{production: []string{"local", "devlocal", "buildlocal", "serde_json"}}); strings.Join(got.production, ",") != "local,devlocal" {
		t.Errorf("example locals = %+v, want [local devlocal]", got.production)
	}
	if got := localCargoBuildImports(c, scoped, targetImports{production: []string{"local", "buildlocal"}}); strings.Join(got.production, ",") != "buildlocal" {
		t.Errorf("build locals = %+v, want [buildlocal]", got.production)
	}
	if got := localCargoBuildImports(c, scoped, targetImports{test: []string{"buildlocal"}}); strings.Join(got.test, ",") != "buildlocal" {
		t.Errorf("build test locals = %+v, want [buildlocal]", got.test)
	}
}

func TestImportOverrideScopes(t *testing.T) {
	manifest := &cargoManifest{
		packageName: "demo",
		normalDeps:  map[string]cargoDependency{},
		devDeps:     map[string]cargoDependency{},
		buildDeps:   map[string]cargoDependency{},
	}
	mapped := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "rust xmapped //pkg:target"}})
	if err := validateCargoImports(mapped, manifest, libraryKind, targetImports{production: []string{"xmapped"}}); err != nil {
		t.Errorf("mapped production import rejected: %v", err)
	}
	if err := validateExampleImports(mapped, manifest, targetImports{production: []string{"xmapped"}, test: []string{"xmapped"}}); err != nil {
		t.Errorf("mapped example import rejected: %v", err)
	}
	if err := validateBuildImports(mapped, manifest, targetImports{production: []string{"xmapped"}, test: []string{"xmapped"}}); err != nil {
		t.Errorf("mapped build import rejected: %v", err)
	}
	if got := localCargoExampleImports(mapped, manifest, targetImports{production: []string{"xmapped"}}); strings.Join(got.production, ",") != "xmapped" {
		t.Errorf("mapped example local = %+v", got.production)
	}
	if got := localCargoBuildImports(mapped, manifest, targetImports{production: []string{"xmapped"}, test: []string{"xmapped"}}); strings.Join(got.production, ",") != "xmapped" || strings.Join(got.test, ",") != "xmapped" {
		t.Errorf("mapped build locals = %+v", got)
	}
	conflict := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "rust xmapped //pkg:target"}})
	conflict.Exts[languageName] = &rustConfig{IgnoreConfig: common.IgnoreConfig{Ignores: []*common.IgnoreEntry{{Value: "xmapped"}}}}
	if err := validateCargoImports(conflict, manifest, libraryKind, targetImports{production: []string{"xmapped"}}); err == nil || !strings.Contains(err.Error(), "both") {
		t.Errorf("production mapping/ignore conflict err = %v", err)
	}
	if err := validateExampleImports(conflict, manifest, targetImports{production: []string{"xmapped"}}); err == nil || !strings.Contains(err.Error(), "both") {
		t.Errorf("example production mapping/ignore conflict err = %v", err)
	}
	if err := validateExampleImports(conflict, manifest, targetImports{test: []string{"xmapped"}}); err == nil || !strings.Contains(err.Error(), "both") {
		t.Errorf("example mapping/ignore conflict err = %v", err)
	}
	if err := validateBuildImports(conflict, manifest, targetImports{production: []string{"xmapped"}}); err == nil || !strings.Contains(err.Error(), "both") {
		t.Errorf("build mapping/ignore conflict err = %v", err)
	}
}

func TestSetScriptAttrsScopes(t *testing.T) {
	manifest := &cargoManifest{
		packageName: "demo",
		buildDeps: map[string]cargoDependency{
			"cc":    {external: true},
			"local": {},
		},
	}
	r := rule.NewRule(scriptKind, "demo_build_script")
	setScriptAttrs(r, "crates/demo", manifest)
	if got := cargoCallNames(r.Attr("deps")); len(got) != 1 || got[0] != "cc" {
		t.Errorf("script deps = %v, want [cc]", got)
	}
}

func TestSiblingLibFlavorFallback(t *testing.T) {
	ordinary := &cargoManifest{targets: []cargoTarget{
		{kind: libraryKind, name: "flavored", flavor: "proc-macro"},
		{kind: libraryKind, name: "plain"},
		{kind: binaryKind, name: "tool"},
	}}
	if got := siblingLibName(ordinary, cargoTarget{kind: binaryKind, name: "tool"}); got != "plain" {
		t.Errorf("sibling prefers ordinary lib = %q, want plain", got)
	}
	flavored := &cargoManifest{targets: []cargoTarget{
		{kind: libraryKind, name: "flavored", flavor: "proc-macro"},
		{kind: binaryKind, name: "tool"},
	}}
	if got := siblingLibName(flavored, cargoTarget{kind: binaryKind, name: "tool"}); got != "flavored" {
		t.Errorf("sibling falls back to flavored lib = %q, want flavored", got)
	}
}

func TestGenerateCargoBuildScript(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "Cargo.toml", "[package]\nname = \"scripted\"\nversion = \"0.5.0\"\nedition = \"2021\"\nbuild = \"build/script.rs\"\n[dependencies]\nserde_json = \"1\"\n[build-dependencies]\ncc = \"1\"\n[lib]\nname = \"scripted_lib\"\npath = \"source/lib.rs\"\n")
	writeFixture(t, root, "source/lib.rs", "use serde_json::Value;\npub fn value() -> Value {\n    Value::Null\n}\n")
	writeFixture(t, root, "build/script.rs", "use cc::Build;\nfn main() {\n    let _ = Build::new();\n}\n")
	l := &rustLang{}
	result := l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          root,
		RegularFiles: []string{"Cargo.toml", "source/lib.rs", "build/script.rs"},
	})
	if len(l.errors) != 0 {
		t.Fatalf("script generation errors = %v", l.errors)
	}
	var script *rule.Rule
	for _, r := range result.Gen {
		if r.Kind() == scriptKind {
			script = r
		}
	}
	if script == nil {
		t.Fatalf("no %s rule in %+v", scriptKind, result.Gen)
	}
	if script.Name() != "scripted_build_script" || script.AttrString("crate_root") != "build/script.rs" || script.AttrString("version") != "0.5.0" || script.AttrString("pkg_name") != "scripted" {
		t.Errorf("script rule = %+v", script)
	}
	if cc, ok := script.Attr("use_cc_toolchain").(*bzl.LiteralExpr); !ok || cc.Token != "1" {
		t.Errorf("script use_cc_toolchain = %v, want 1", script.Attr("use_cc_toolchain"))
	}
	if shell, ok := script.Attr("use_default_shell_env").(*bzl.LiteralExpr); !ok || shell.Token != "0" {
		t.Errorf("script use_default_shell_env = %v, want 0", script.Attr("use_default_shell_env"))
	}
	if !script.AttrBool("emit_warnings") {
		t.Errorf("script emit_warnings = %v, want True", script.Attr("emit_warnings"))
	}
	for _, imports := range result.Imports {
		if raw, ok := imports.(targetImports); ok && len(raw.production) > 0 {
			if raw.scriptDep != ":scripted_build_script" {
				t.Errorf("consumer scriptDep = %q, want :scripted_build_script", raw.scriptDep)
			}
		}
	}

	badRoot := t.TempDir()
	writeFixture(t, badRoot, "Cargo.toml", "[package]\nname = \"bad\"\nbuild = \"build.rs\"\n[dependencies]\nserde_json = \"1\"\n")
	writeFixture(t, badRoot, "build.rs", "use serde_json::Value;\nfn main() {\n    let _ = Value::Null;\n}\n")
	bad := &rustLang{}
	bad.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: badRoot}, Dir: badRoot, RegularFiles: []string{"Cargo.toml", "build.rs"}})
	if len(bad.errors) != 1 || !strings.Contains(bad.errors[0], "build-dependencies") {
		t.Errorf("build scope errors = %v", bad.errors)
	}
}

func TestBuildScriptUserAttrsPreserved(t *testing.T) {
	for _, attr := range []string{"data", "tools", "build_script_env", "build_script_env_files", "toolchains"} {
		if kindInfo().MergeableAttrs[attr] {
			t.Errorf("MergeableAttrs[%q] = true, want false (user-owned with # keep)", attr)
		}
	}
	root := t.TempDir()
	writeFixture(t, root, "Cargo.toml", "[package]\nname = \"scripted\"\nversion = \"0.5.0\"\nedition = \"2021\"\nbuild = \"build/script.rs\"\n[build-dependencies]\ncc = \"1\"\n[lib]\nname = \"scripted_lib\"\npath = \"source/lib.rs\"\n")
	writeFixture(t, root, "source/lib.rs", "pub fn value() {}\n")
	writeFixture(t, root, "build/script.rs", "fn main() {}\n")
	l := &rustLang{}
	result := l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          root,
		RegularFiles: []string{"Cargo.toml", "source/lib.rs", "build/script.rs"},
	})
	if len(l.errors) != 0 {
		t.Fatalf("script generation errors = %v", l.errors)
	}
	for _, r := range result.Gen {
		if r.Kind() != scriptKind {
			continue
		}
		for _, attr := range []string{"data", "tools", "build_script_env", "build_script_env_files", "toolchains"} {
			if r.Attr(attr) != nil {
				t.Errorf("generated %s sets %q, want absent (user-owned)", r.Name(), attr)
			}
		}
	}
}

func TestEmitBuildScriptInvalidPackageName(t *testing.T) {
	l := &rustLang{}
	result := &language.GenerateResult{}
	l.emitBuildScript(
		language.GenerateArgs{Config: &config.Config{}},
		"Cargo.toml",
		&cargoManifest{packageName: "---"},
		map[string]bool{},
		func(string) ([]byte, error) { return nil, nil },
		map[string]string{},
		result,
	)
	if got := strings.Join(l.errors, "\n"); !strings.Contains(got, "normalizes to an empty target name") {
		t.Fatalf("errors = %q, want normalization error", got)
	}
	if len(result.Gen) != 0 || len(result.Imports) != 0 {
		t.Errorf("result = %+v, want nothing emitted", result)
	}
}

func TestGenerateCargoSliceFailures(t *testing.T) {
	cases := []struct {
		name     string
		manifest string
		files    map[string]string
		want     string
	}{
		{
			name:     "example undeclared import",
			manifest: "[package]\nname = \"app\"\n[[example]]\nname = \"demo\"\npath = \"examples/demo.rs\"\n",
			files:    map[string]string{"examples/demo.rs": "use missing_crate::Thing;\nfn main() {}\n"},
			want:     "declare it in [dependencies] or [dev-dependencies]",
		},
		{
			name:     "missing build script",
			manifest: "[package]\nname = \"app\"\nbuild = \"build/missing.rs\"\n[lib]\npath = \"src/lib.rs\"\n",
			files:    map[string]string{"src/lib.rs": ""},
			want:     "build script build/missing.rs does not exist",
		},
		{
			name:     "broken build script",
			manifest: "[package]\nname = \"app\"\nbuild = \"build.rs\"\n[lib]\npath = \"src/lib.rs\"\n",
			files: map[string]string{
				"src/lib.rs": "",
				"build.rs":   "mod missing;\nfn main() {}\n",
			},
			want: "orphan module",
		},
		{
			name:     "script steals lib sources",
			manifest: "[package]\nname = \"app\"\nbuild = \"src/lib.rs\"\n[lib]\npath = \"src/lib.rs\"\n",
			files:    map[string]string{"src/lib.rs": ""},
			want:     "owned by both",
		},
		{
			name:     "path version mismatch",
			manifest: "[package]\nname = \"app\"\n[dependencies]\nhelper = { path = \"helpers\", version = \"^9\" }\n[lib]\npath = \"src/lib.rs\"\n",
			files: map[string]string{
				"src/lib.rs":         "",
				"helpers/Cargo.toml": "[package]\nname = \"helper\"\nversion = \"0.1.0\"\n",
				"helpers/src/lib.rs": "",
			},
			want: "does not satisfy",
		},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			root := t.TempDir()
			writeFixture(t, root, "Cargo.toml", tc.manifest)
			for name, content := range tc.files {
				writeFixture(t, root, name, content)
			}
			l := &rustLang{}
			l.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: root, RegularFiles: []string{"Cargo.toml"}})
			if len(l.errors) == 0 || !strings.Contains(strings.Join(l.errors, "\n"), tc.want) {
				t.Errorf("errors = %v, want %q", l.errors, tc.want)
			}
		})
	}
}

func TestDxCrateImports(t *testing.T) {
	lang := NewLanguage()
	named := rule.NewRule(dxCrateKind, "dx_digest")
	named.SetAttr("crate_name", "digest_crate")
	if got := lang.Imports(&config.Config{}, named, nil); len(got) != 1 || got[0].Imp != "digest_crate" {
		t.Errorf("named macro imports = %+v, want [digest_crate]", got)
	}
	bare := rule.NewRule(dxCrateKind, "dx_atomic_fs")
	if got := lang.Imports(&config.Config{}, bare, nil); len(got) != 1 || got[0].Imp != "dx_atomic_fs" {
		t.Errorf("bare macro imports = %+v, want [dx_atomic_fs]", got)
	}
	if got := lang.Imports(&config.Config{}, rule.NewRule(binaryKind, "tool"), nil); got != nil {
		t.Errorf("binary imports = %+v, want nil", got)
	}
}

func TestDxCrateNames(t *testing.T) {
	if got := dxCrateNames(nil); len(got) != 0 {
		t.Errorf("nil file names = %+v, want empty", got)
	}
	empty := rule.EmptyFile("BUILD.bazel", "pkg")
	if got := dxCrateNames(empty); len(got) != 0 {
		t.Errorf("empty file names = %+v, want empty", got)
	}
	mixed := rule.EmptyFile("BUILD.bazel", "pkg")
	mixed.Rules = append(mixed.Rules,
		rule.NewRule(libraryKind, "plain"),
		rule.NewRule(dxCrateKind, "dx_a"),
		rule.NewRule(dxCrateKind, "dx_b"),
	)
	got := dxCrateNames(mixed)
	if len(got) != 2 || !got["dx_a"] || !got["dx_b"] {
		t.Errorf("mixed names = %+v, want [dx_a dx_b]", got)
	}
}

func TestFilterDxCrateCovered(t *testing.T) {
	file := rule.EmptyFile("BUILD.bazel", "pkg")
	file.Rules = append(file.Rules, rule.NewRule(dxCrateKind, "dx_a"))
	lib := rule.NewRule(libraryKind, "dx_a")
	unit := rule.NewRule(testKind, "dx_a_test")
	unit.SetAttr("crate", ":dx_a")
	integration := rule.NewRule(testKind, "smoke_test")
	flavored := rule.NewRule(procMacroKind, "dx_a")
	binary := rule.NewRule(binaryKind, "tool")
	otherLib := rule.NewRule(libraryKind, "other")
	otherUnit := rule.NewRule(testKind, "other_test")
	otherUnit.SetAttr("crate", ":other")
	result := filterDxCrateCovered(file, language.GenerateResult{
		Gen:     []*rule.Rule{lib, unit, integration, flavored, binary, otherLib, otherUnit},
		Imports: []interface{}{"lib", "unit", "integration", "flavored", "binary", "otherLib", "otherUnit"},
	})
	var names []string
	for _, r := range result.Gen {
		names = append(names, r.Kind()+":"+r.Name())
	}
	want := testKind + ":smoke_test," + procMacroKind + ":dx_a," + binaryKind + ":tool," + libraryKind + ":other," + testKind + ":other_test"
	if strings.Join(names, ",") != want {
		t.Errorf("filtered = %q, want %q", strings.Join(names, ","), want)
	}
	if len(result.Imports) != len(result.Gen) {
		t.Fatalf("imports = %d, want %d", len(result.Imports), len(result.Gen))
	}
	if result.Imports[0] != "integration" || result.Imports[3] != "otherLib" {
		t.Errorf("imports = %+v, want kept entries only", result.Imports)
	}
	plain := language.GenerateResult{Gen: []*rule.Rule{rule.NewRule(libraryKind, "x")}}
	if out := filterDxCrateCovered(nil, plain); len(out.Gen) != 1 {
		t.Errorf("nil file filtered %d rules, want 1", len(out.Gen))
	}
	if out := filterDxCrateCovered(file, language.GenerateResult{}); len(out.Gen) != 0 {
		t.Errorf("empty result filtered %d rules, want 0", len(out.Gen))
	}
}

func TestCheckExistingClaimsDxCrate(t *testing.T) {
	file := rule.EmptyFile("BUILD.bazel", "pkg")
	file.Rules = append(file.Rules, rule.NewRule(dxCrateKind, "dx_a"))
	if err := checkExistingClaims(file, nil, []*rule.Rule{rule.NewRule(libraryKind, "dx_a")}); err != nil {
		t.Errorf("macro-owned library rejected: %v", err)
	}
	flavored := rule.NewRule(procMacroKind, "dx_a")
	if err := checkExistingClaims(file, nil, []*rule.Rule{flavored}); err == nil {
		t.Error("macro/flavored collision accepted")
	}
	if err := checkExistingClaims(file, nil, []*rule.Rule{rule.NewRule(libraryKind, "other")}); err != nil {
		t.Errorf("unique generated target rejected: %v", err)
	}
	colliding := rule.EmptyFile("BUILD.bazel", "pkg")
	colliding.Rules = append(colliding.Rules, rule.NewRule("filegroup", "dx_a"))
	if err := checkExistingClaims(colliding, nil, []*rule.Rule{rule.NewRule(libraryKind, "dx_a")}); err == nil {
		t.Error("filegroup/library collision accepted")
	}
}

func TestRustLoadsKeepDxCrateOffThePublicModule(t *testing.T) {
	l := &rustLang{}
	found := false
	for _, load := range l.Loads() {
		if strings.HasSuffix(load.Name, "//rust/rules:defs.bzl") {
			for _, sym := range load.Symbols {
				if sym == dxCrateKind {
					t.Errorf("%s exports %q", load.Name, dxCrateKind)
				}
			}
		}
		if strings.HasSuffix(load.Name, "//rust/rules:crate.bzl") {
			for _, sym := range load.Symbols {
				if sym == dxCrateKind {
					found = true
				}
			}
		}
	}
	if !found {
		t.Errorf("Loads() symbols = %+v, want %q on //rust/rules:crate.bzl", l.Loads(), dxCrateKind)
	}
}

func TestGenerateCargoExampleScopes(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "Cargo.toml", "[package]\nname = \"app\"\n[dev-dependencies]\nhelper = { path = \"helpers\" }\n[[example]]\nname = \"demo\"\npath = \"examples/demo.rs\"\ntest = true\n")
	writeFixture(t, root, "helpers/Cargo.toml", "[package]\nname = \"helper\"\n")
	writeFixture(t, root, "helpers/src/lib.rs", "")
	writeFixture(t, root, "examples/demo.rs", "use helper::thing;\nfn main() {}\n#[test]\nfn demo_runs() {}\n")
	l := &rustLang{}
	result := l.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: root, RegularFiles: []string{"Cargo.toml"}})
	if len(l.errors) != 0 {
		t.Fatalf("example generation errors = %v", l.errors)
	}
	var binary, wrapper *rule.Rule
	for _, r := range result.Gen {
		switch r.Name() {
		case "demo_example":
			binary = r
		case "demo_example_test":
			wrapper = r
		}
	}
	if binary == nil || binary.Kind() != binaryKind {
		t.Fatalf("demo example binary = %+v", result.Gen)
	}
	if wrapper == nil || wrapper.AttrString("crate") != ":demo_example" {
		t.Errorf("demo example wrapper = %+v", wrapper)
	}
	found := false
	for _, raw := range result.Imports {
		if imports, ok := raw.(targetImports); ok {
			for _, name := range imports.production {
				if name == "helper" {
					found = true
				}
			}
		}
	}
	if !found {
		t.Errorf("example first-party edge missing in %+v", result.Imports)
	}
}
