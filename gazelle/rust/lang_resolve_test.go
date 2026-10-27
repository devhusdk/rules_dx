package rust

import (
	"path/filepath"
	"sort"
	"strings"
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
	bzl "github.com/bazelbuild/buildtools/build"
	"github.com/devhusdk/rules_dx/gazelle/common"
)

func TestResolveMirrorPaths(t *testing.T) {
	l := &rustLang{}
	index := resolverIndex(l, struct{ pkg, name string }{"lib/b", "b"})
	cfg := resolverConfig(t, nil)
	cfg.Exts[languageName] = &rustConfig{}
	mirrored := rule.NewRule(binaryKind, "app")
	l.Resolve(cfg, index, nil, mirrored, targetImports{mirrorPaths: []string{"b"}}, label.New("", "app", "app"))
	if got := strings.Join(mirrored.AttrStrings("deps"), ","); got != "//lib/b" {
		t.Errorf("mirrored deps = %q, want //lib/b", got)
	}
	silent := rule.NewRule(binaryKind, "quiet")
	l.Resolve(cfg, index, nil, silent, targetImports{mirrorPaths: []string{"missing"}}, label.New("", "app", "quiet"))
	if silent.Attr("deps") != nil {
		t.Errorf("silent mirror emitted deps: %v", silent.AttrStrings("deps"))
	}
	own := rule.NewRule(libraryKind, "b")
	l.Resolve(cfg, index, nil, own, targetImports{mirrorPaths: []string{"b"}}, label.New("", "lib/b", "b"))
	if own.Attr("deps") != nil {
		t.Errorf("self mirror emitted deps: %v", own.AttrStrings("deps"))
	}
	ignored := &common.IgnoreEntry{Value: "b"}
	cfg.Exts[languageName] = &rustConfig{IgnoreConfig: common.IgnoreConfig{Ignores: []*common.IgnoreEntry{ignored}}}
	skipped := rule.NewRule(binaryKind, "skipped")
	l.Resolve(cfg, index, nil, skipped, targetImports{mirrorPaths: []string{"b"}}, label.New("", "app", "skipped"))
	if skipped.Attr("deps") != nil || !ignored.Used() {
		t.Errorf("ignored mirror = deps:%v used:%v", skipped.Attr("deps"), ignored.Used())
	}
	if len(l.errors) != 0 {
		t.Errorf("mirror errors = %v", l.errors)
	}
}

func TestGenerateCargoLibraryVisibility(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "Cargo.toml", "[package]\nname = \"app\"\n")
	writeFixture(t, root, "src/lib.rs", "")
	writeFixture(t, root, "src/main.rs", "fn main() {}\n")
	l := &rustLang{}
	result := l.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: root, RegularFiles: []string{"Cargo.toml"}})
	if len(l.errors) != 0 {
		t.Fatalf("generation errors = %v", l.errors)
	}
	for _, r := range result.Gen {
		switch r.Kind() {
		case libraryKind:
			if got := strings.Join(r.AttrStrings("visibility"), ","); got != "//visibility:public" {
				t.Errorf("lib visibility = %q, want //visibility:public", got)
			}
		case binaryKind:
			if r.Attr("visibility") != nil {
				t.Errorf("bin visibility = %v, want none", r.Attr("visibility"))
			}
		}
	}
	f := rule.EmptyFile("BUILD.bazel", "")
	pkg := rule.NewRule("package", "")
	pkg.SetAttr("default_visibility", []string{"//visibility:public"})
	f.Rules = append(f.Rules, pkg)
	owned := &rustLang{}
	kept := owned.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: root, RegularFiles: []string{"Cargo.toml"}, File: f})
	if len(owned.errors) != 0 {
		t.Fatalf("generation errors = %v", owned.errors)
	}
	for _, r := range kept.Gen {
		if isLibraryKind(r.Kind()) && r.Attr("visibility") != nil {
			t.Errorf("%s visibility = %v, want none under file default", r.Name(), r.Attr("visibility"))
		}
	}
}

func TestSourceFilesBoundaries(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "src/lib.rs", "")
	writeFixture(t, root, "src/.hidden/no.rs", "")
	writeFixture(t, root, "src/nested/BUILD.bazel", "")
	writeFixture(t, root, "src/nested/no.rs", "")
	writeFixture(t, root, "src/legacy/BUILD", "")
	writeFixture(t, root, "src/legacy/no.rs", "")
	writeFixture(t, root, "tests/smoke.rs", "")
	files, err := sourceFiles(root, "pkg")
	if err != nil {
		t.Fatal(err)
	}
	if got := strings.Join(files, ","); got != "pkg/src/lib.rs,pkg/tests/smoke.rs" {
		t.Errorf("source files = %q", got)
	}
}

func TestGenerateEmptyNoRootAndSourceError(t *testing.T) {
	l := &rustLang{}
	root := t.TempDir()
	if result := l.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: root}); len(result.Gen) != 0 {
		t.Errorf("empty generation = %+v", result)
	}
	writeFixture(t, root, "src/helper.rs", "")
	if result := l.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: root}); len(result.Gen) != 0 {
		t.Errorf("no-root generation = %+v", result)
	}
	missing := filepath.Join(root, "gone")
	if _, err := sourceFiles(missing, "gone"); err == nil {
		t.Error("missing source directory accepted")
	}
	failed := &rustLang{}
	failed.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: missing, Rel: "gone"})
	if len(failed.errors) != 1 {
		t.Errorf("GenerateRules source error = %v", failed.errors)
	}
	notDir := filepath.Join(root, "plain-file")
	writeFixture(t, root, "plain-file", "content")
	if _, err := sourceFiles(notDir, "file"); err != nil {
		t.Errorf("single-file scan failed: %v", err)
	}
	bad := t.TempDir()
	writeFixture(t, bad, "src/lib.rs", "mod missing;\n")
	broken := &rustLang{}
	broken.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: bad}, Dir: bad})
	if len(broken.errors) != 1 || !strings.Contains(broken.errors[0], "orphan module") {
		t.Errorf("source parse errors = %v", broken.errors)
	}
}

func TestGenerateCargoImplicitFailure(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "Cargo.toml", "[package]\nname = \"app\"\n")
	writeFixture(t, root, "tests/---.rs", "")
	l := &rustLang{}
	l.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: root, RegularFiles: []string{"Cargo.toml"}})
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "empty target name") {
		t.Errorf("implicit target errors = %v", l.errors)
	}
}

func TestGenerateExistingNameCollisions(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "crates/demo/src/lib.rs", "")
	f := rule.EmptyFile("BUILD.bazel", "crates/demo")
	f.Rules = append(f.Rules, rule.NewRule("filegroup", "demo"))
	l := &rustLang{}
	l.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: filepath.Join(root, "crates/demo"), Rel: "crates/demo", File: f})
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "existing filegroup") {
		t.Errorf("source collision errors = %v", l.errors)
	}

	cargoRoot := t.TempDir()
	writeFixture(t, cargoRoot, "Cargo.toml", "[package]\nname = \"app\"\n")
	writeFixture(t, cargoRoot, "src/lib.rs", "")
	cargoFile := rule.EmptyFile("BUILD.bazel", "")
	cargoFile.Rules = append(cargoFile.Rules, rule.NewRule("filegroup", "app"))
	cargo := &rustLang{}
	cargo.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: cargoRoot}, Dir: cargoRoot, RegularFiles: []string{"Cargo.toml"}, File: cargoFile})
	if len(cargo.errors) != 1 || !strings.Contains(cargo.errors[0], "existing filegroup") {
		t.Errorf("Cargo collision errors = %v", cargo.errors)
	}
}

func TestCargoImportValidationAndExpressions(t *testing.T) {
	manifest := &cargoManifest{
		packageName: "app-name",
		normalDeps:  map[string]cargoDependency{"normal": {external: true}, "local": {}},
		devDeps:     map[string]cargoDependency{"dev": {external: true}, "local_dev": {}},
	}
	imports := targetImports{production: []string{"normal", "local"}, test: []string{"dev", "local_dev"}}
	cfg := &config.Config{}
	if err := validateCargoImports(cfg, manifest, libraryKind, imports); err != nil {
		t.Fatal(err)
	}
	if err := validateCargoImports(cfg, manifest, libraryKind, targetImports{production: []string{"unknown"}}); err == nil {
		t.Error("unknown production import accepted")
	}
	if err := validateCargoImports(cfg, manifest, testKind, targetImports{production: []string{"dev"}}); err != nil {
		t.Errorf("test dev import rejected: %v", err)
	}
	if err := validateCargoImports(cfg, manifest, testKind, targetImports{test: []string{"unknown"}}); err == nil {
		t.Error("unknown test import accepted")
	}
	if err := validateCargoImports(cfg, manifest, libraryKind, targetImports{production: []string{"app_name"}}); err != nil {
		t.Errorf("self import rejected: %v", err)
	}
	if err := validateExampleImports(cfg, manifest, targetImports{production: []string{"app_name"}}); err != nil {
		t.Errorf("example self import rejected: %v", err)
	}
	bad := &cargoManifest{packageName: "a.b"}
	if err := validateCargoImports(cfg, bad, libraryKind, targetImports{production: []string{"x"}}); err == nil || !strings.Contains(err.Error(), "not a Cargo crate name") {
		t.Errorf("bad package name error = %v", err)
	}
	local := localCargoImports(cfg, manifest, imports, true)
	if strings.Join(local.production, ",") != "local" || strings.Join(local.test, ",") != "local_dev" {
		t.Errorf("local imports = %+v", local)
	}
	r := rule.NewRule(libraryKind, "app")
	setCargoAttrs(r, "pkg/app", manifest, true)
	if r.Attr("deps") == nil || r.Attr("aliases") == nil {
		t.Errorf("Cargo expressions missing")
	}
	call := cargoCall("aliases", "pkg/app", true)
	if call.Merge(nil) == nil || (crateDepsCall{call: cargoCall("all_crate_deps", "pkg/app", false), external: true}).Merge(nil) == nil {
		t.Error("generated expressions did not merge")
	}
	withoutDev := localCargoImports(cfg, manifest, imports, false)
	if len(withoutDev.test) != 0 {
		t.Errorf("production local imports include dev: %+v", withoutDev)
	}
	empty := rule.NewRule(libraryKind, "empty")
	setCargoAttrs(empty, "pkg/app", manifest, false)
	if empty.Attr("deps") == nil || empty.Attr("aliases") == nil {
		t.Errorf("all-declared Cargo attrs = deps:%v aliases:%v", empty.Attr("deps"), empty.Attr("aliases"))
	}
	dashedManifest := &cargoManifest{
		packageName: "app",
		normalDeps:  map[string]cargoDependency{"quick_xml": {external: true, label: "quick-xml"}, "serde_json": {external: true, label: "serde_json"}},
		devDeps:     map[string]cargoDependency{},
	}
	dashed := rule.NewRule(libraryKind, "dashed")
	setCargoAttrs(dashed, "pkg/app", dashedManifest, false)
	if got := exprString(dashed.Attr("deps")); got != `all_crate_deps(normal = True, package_name = "pkg/app")` {
		t.Errorf("dashed deps = %s", got)
	}
}

func exprString(e bzl.Expr) string {
	return strings.TrimSpace(bzl.FormatString(&bzl.File{Stmt: []bzl.Expr{e}}))
}

func TestCargoAttrsUseAllCrateDeps(t *testing.T) {
	manifest, err := parseCargoManifest("Cargo.toml", []byte(`[package]
name = "app"
version = "0.1.0"

[target.'cfg(unix)'.dependencies]
libc = "0.2"
`))
	if err != nil {
		t.Fatal(err)
	}
	lib := rule.NewRule(libraryKind, "app")
	setCargoAttrs(lib, "crates/app", manifest, false)
	if got := exprString(lib.Attr("deps")); got != `all_crate_deps(normal = True, package_name = "crates/app")` {
		t.Errorf("cfg deps = %s", got)
	}
	test := rule.NewRule(testKind, "app_test")
	setCargoAttrs(test, "crates/app", manifest, true)
	if got := exprString(test.Attr("deps")); got != `all_crate_deps(normal = True, package_name = "crates/app", normal_dev = True)` {
		t.Errorf("test deps = %s", got)
	}
	devOnly := &cargoManifest{packageName: "app", devDeps: map[string]cargoDependency{"x": {external: true}}}
	prod := rule.NewRule(libraryKind, "app")
	setCargoAttrs(prod, "crates/app", devOnly, false)
	if got := exprString(prod.Attr("deps")); got != "[]" {
		t.Errorf("dev-only library deps = %s, want []", got)
	}
}

func TestLegacyCrateDepsMigrates(t *testing.T) {
	call := crateDepsCall{call: cargoCall("all_crate_deps", "dx/dx_a", false), external: true}
	merged := call.Merge(crateDepsFileExpr([]string{"serde_json"}, "dx/dx_a", "//x"))
	if got := exprString(merged); got != `all_crate_deps(normal = True, package_name = "dx/dx_a") + ["//x"]` {
		t.Errorf("migrated deps = %s", got)
	}
}

func TestCargoImportResolveOverrides(t *testing.T) {
	manifest := &cargoManifest{
		packageName: "demo",
		normalDeps:  map[string]cargoDependency{},
		devDeps:     map[string]cargoDependency{},
	}
	c := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "rust mapped //pkg:target"}})
	mapped := targetImports{production: []string{"mapped"}}
	if err := validateCargoImports(c, manifest, libraryKind, mapped); err != nil {
		t.Errorf("mapped import rejected: %v", err)
	}
	if err := validateCargoImports(c, manifest, libraryKind, targetImports{production: []string{"mapped", "unknown"}}); err == nil || !strings.Contains(err.Error(), `"unknown"`) {
		t.Errorf("unmapped import not reported: %v", err)
	}
	local := localCargoImports(c, manifest, mapped, false)
	if strings.Join(local.production, ",") != "mapped" {
		t.Errorf("mapped local imports = %+v, want [mapped]", local)
	}
	c.Exts[languageName] = &rustConfig{IgnoreConfig: common.IgnoreConfig{Ignores: []*common.IgnoreEntry{{Value: "mapped"}}}}
	if err := validateCargoImports(c, manifest, libraryKind, mapped); err == nil || !strings.Contains(err.Error(), "both") {
		t.Errorf("mapping/ignore conflict not reported: %v", err)
	}
}

func TestResolvePreservesCrateDeps(t *testing.T) {
	l := &rustLang{}
	resolveSibling := func(r *rule.Rule) {
		l.Resolve(resolverConfig(t, nil), resolverIndex(l), nil, r,
			targetImports{siblingLib: "core"}, label.New("", "pkg", "tool"))
	}
	fresh := rule.NewRule(binaryKind, "tool")
	resolveSibling(fresh)
	if got := strings.Join(fresh.AttrStrings("deps"), ","); got != ":core" {
		t.Errorf("fresh deps = %q, want :core", got)
	}
	listed := rule.NewRule(binaryKind, "tool")
	listed.SetAttr("deps", []string{":other"})
	resolveSibling(listed)
	if got := strings.Join(listed.AttrStrings("deps"), ","); got != ":core,:other" {
		t.Errorf("listed deps = %q, want :core,:other", got)
	}
	called := rule.NewRule(binaryKind, "tool")
	called.SetAttr("deps", crateDepsCall{call: cargoCall("all_crate_deps", "pkg", false), external: true})
	resolveSibling(called)
	concat, ok := called.Attr("deps").(*bzl.BinaryExpr)
	if !ok || concat.Op != "+" {
		t.Fatalf("concat deps = %#v, want crate_deps + labels", called.Attr("deps"))
	}
	if _, ok := concat.X.(*bzl.CallExpr); !ok {
		t.Errorf("concat base = %T, want crate_deps call", concat.X)
	}
	var extra []string
	if list, ok := concat.Y.(*bzl.ListExpr); ok {
		for _, item := range list.List {
			if s, ok := item.(*bzl.StringExpr); ok {
				extra = append(extra, s.Value)
			}
		}
	}
	if strings.Join(extra, ",") != ":core" {
		t.Errorf("concat extra = %q, want :core", strings.Join(extra, ","))
	}
	merged := (depsConcatExpr{base: concat.X, extra: []string{":core"}}).Merge(nil)
	if merged == nil {
		t.Error("concat merge returned nil")
	}
}

func TestResolveDedupesSiblingLibDuplicate(t *testing.T) {
	l := &rustLang{}
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "rust quality_result //quality/result:quality_result"}})
	r := rule.NewRule(binaryKind, "print_result")
	l.Resolve(cfg, resolverIndex(l), nil, r,
		targetImports{production: []string{"quality_result"}, siblingLib: "quality_result"},
		label.New("rules_dx", "quality/result", "print_result"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != ":quality_result" {
		t.Errorf("deps = %q, want :quality_result", got)
	}
	from := label.New("", "quality/result", "print_result")
	deps := map[string]bool{
		"//other/pkg:quality_result":      true,
		"@third//quality/result:tool":     true,
		"//quality/result:quality_result": true,
	}
	addLocalDep(deps, from, "quality_result")
	var got []string
	for dep := range deps {
		got = append(got, dep)
	}
	sort.Strings(got)
	want := "//other/pkg:quality_result,:quality_result,@third//quality/result:tool"
	if strings.Join(got, ",") != want {
		t.Errorf("deps = %q, want %q", strings.Join(got, ","), want)
	}
}

func TestCrateUniversePackage(t *testing.T) {
	for _, tc := range []struct {
		path, pkg, want string
	}{
		{"dx/output", "dx_output", "dx/dx_output"},
		{"rust/tests/fixtures/hello", "hello", "rust/tests/fixtures/hello"},
		{"crates/app", "cargo-app", "crates/cargo-app"},
		{"root", "root", "root"},
		{"dx/output", "", "dx/output"},
	} {
		var manifest *cargoManifest
		if tc.pkg != "" || tc.path == "dx/output" {
			manifest = &cargoManifest{packageName: tc.pkg}
		}
		if got := crateUniversePackage(tc.path, manifest); got != tc.want {
			t.Errorf("crateUniversePackage(%q, %q) = %q, want %q", tc.path, tc.pkg, got, tc.want)
		}
	}
	if got := crateUniversePackage("dx/output", nil); got != "dx/output" {
		t.Errorf("nil manifest = %q, want path fallback", got)
	}
}

func strListExpr(values ...string) *bzl.ListExpr {
	list := make([]bzl.Expr, len(values))
	for i, v := range values {
		list[i] = &bzl.StringExpr{Value: v}
	}
	return &bzl.ListExpr{List: list}
}

func crateDepsFileExpr(names []string, packageName string, tail ...string) bzl.Expr {
	call := &bzl.CallExpr{X: &bzl.Ident{Name: "crate_deps"}, List: []bzl.Expr{
		strListExpr(names...),
		&bzl.AssignExpr{LHS: &bzl.Ident{Name: "package_name"}, Op: "=", RHS: &bzl.StringExpr{Value: packageName}},
	}}
	if len(tail) == 0 {
		return call
	}
	return &bzl.BinaryExpr{X: call, Op: "+", Y: strListExpr(tail...)}
}

func tailStrings(t *testing.T, e bzl.Expr) []string {
	t.Helper()
	bin, ok := e.(*bzl.BinaryExpr)
	if !ok || bin.Op != "+" {
		t.Fatalf("merged deps is %T, want + concat", e)
	}
	list, ok := bin.Y.(*bzl.ListExpr)
	if !ok {
		t.Fatalf("concat tail is %T, want list", bin.Y)
	}
	var out []string
	for _, item := range list.List {
		s, ok := item.(*bzl.StringExpr)
		if !ok {
			t.Fatalf("tail item is %T, want string", item)
		}
		out = append(out, s.Value)
	}
	return out
}

func TestDepsMergePreservesHandLabels(t *testing.T) {
	src := rule.NewRule("rust_binary", "env")
	src.SetAttr("deps", depsConcatExpr{
		base:  crateDepsCall{call: cargoCall("all_crate_deps", "dx/dx_env", false), external: true}.BzlExpr(),
		extra: []string{":dx_env"},
	})
	dst := rule.NewRule("rust_binary", "env")
	dst.SetAttr("deps", crateDepsFileExpr(
		[]string{"blake3", "serde", "serde_json"}, "dx/dx_env",
		":dx_env", "@rules_rust//tools/runfiles:runfiles",
	))
	rule.MergeRules(src, dst, map[string]bool{"deps": true}, "BUILD.bazel")
	got := tailStrings(t, dst.Attr("deps"))
	want := []string{":dx_env", "@rules_rust//tools/runfiles:runfiles"}
	if strings.Join(got, "\x00") != strings.Join(want, "\x00") {
		t.Errorf("merged tail = %q, want %q", got, want)
	}
	again := rule.NewRule("rust_binary", "env")
	again.SetAttr("deps", dst.Attr("deps"))
	rule.MergeRules(src, again, map[string]bool{"deps": true}, "BUILD.bazel")
	if second := tailStrings(t, again.Attr("deps")); strings.Join(second, "\x00") != strings.Join(want, "\x00") {
		t.Errorf("second merge tail = %q, want %q (not idempotent)", second, want)
	}
}

func TestDepsMergeKeepsStaleHandLabels(t *testing.T) {
	src := depsConcatExpr{
		base:  crateDepsCall{call: cargoCall("all_crate_deps", "dx/dx_a", false), external: true}.BzlExpr(),
		extra: []string{":dx_a"},
	}
	merged := src.Merge(crateDepsFileExpr([]string{"serde_json"}, "dx/dx_a", ":dx_a", ":old_lib"))
	if got := tailStrings(t, merged); strings.Join(got, "\x00") != ":dx_a\x00:old_lib" {
		t.Errorf("merged tail = %q, want stale :old_lib preserved", got)
	}
}

func TestCrateDepsCallMerge(t *testing.T) {
	call := crateDepsCall{call: cargoCall("all_crate_deps", "dx/dx_a", false), external: true}
	if merged := call.Merge(nil); !isBareCall(merged) {
		t.Errorf("nil merge is %T, want bare *bzl.CallExpr", merged)
	}
	merged := call.Merge(strListExpr(":hand", "//env:marker_proto_rs"))
	if got := tailStrings(t, merged); strings.Join(got, "\x00") != "//env:marker_proto_rs\x00:hand" {
		t.Errorf("merged tail = %q, want hand labels preserved", got)
	}
	merged = call.Merge(crateDepsFileExpr([]string{"serde_json"}, "dx/dx_a"))
	if _, ok := merged.(*bzl.CallExpr); !ok {
		t.Errorf("fully managed merge is %T, want bare *bzl.CallExpr", merged)
	}
}

func TestCrateWithoutExternalDepsKeepsHandLabels(t *testing.T) {
	manifest := &cargoManifest{
		packageName: "app",
		normalDeps:  map[string]cargoDependency{},
		devDeps:     map[string]cargoDependency{},
	}
	r := rule.NewRule(libraryKind, "app")
	setCargoAttrs(r, "pkg/app", manifest, false)
	if got := exprString(r.Attr("deps")); got != "[]" {
		t.Fatalf("deps attr = %s, want []", got)
	}
	none := crateDepsCall{call: cargoCall("all_crate_deps", "pkg/app", false)}
	merged := none.Merge(crateDepsFileExpr(nil, "pkg/app", "//cli/digest:dx_digest", "//cli/process:dx_process"))
	if got := exprString(merged); got != `["//cli/digest:dx_digest", "//cli/process:dx_process"]` {
		t.Errorf("merged deps = %s, want the hand-written labels kept", got)
	}
}

func isBareCall(e bzl.Expr) bool {
	_, ok := e.(*bzl.CallExpr)
	return ok
}

func TestLangCoverageClosure(t *testing.T) {
	manifest := &cargoManifest{
		packageName: "app",
		normalDeps:  map[string]cargoDependency{"shared": {}},
		devDeps:     map[string]cargoDependency{},
	}
	local := localCargoImports(&config.Config{}, manifest, targetImports{test: []string{"shared"}}, true)
	if strings.Join(local.test, ",") != "shared" {
		t.Errorf("local test imports = %+v, want [shared]", local)
	}
	tree := map[string]*FileFacts{"src/lib.rs": {Externs: []ExternCrate{{Name: "serde"}}}}
	imports := importsFor(tree)
	if strings.Join(imports.production, ",") != "serde" {
		t.Errorf("extern imports = %+v, want [serde]", imports)
	}
	aliased := map[string]*FileFacts{"src/lib.rs": {Externs: []ExternCrate{{Name: "result_proto", As: "proto"}}}}
	aliasedImports := importsFor(aliased)
	if strings.Join(aliasedImports.production, ",") != "result_proto" {
		t.Errorf("aliased extern imports = %+v, want [result_proto]", aliasedImports)
	}
	l := &rustLang{}
	index := resolverIndex(l,
		struct{ pkg, name string }{"lib/prod", "prod"},
		struct{ pkg, name string }{"lib/helpers", "helpers"},
	)
	cfg := resolverConfig(t, nil)
	cfg.Exts[languageName] = &rustConfig{}
	r := rule.NewRule(testKind, "app_test")
	l.Resolve(cfg, index, nil, r, targetImports{production: []string{"prod"}, test: []string{"helpers"}}, label.New("", "app", "app_test"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//lib/helpers,//lib/prod" {
		t.Errorf("test-kind deps = %q", got)
	}
	if len(l.errors) != 0 {
		t.Errorf("test-kind errors = %v", l.errors)
	}
}

func TestImportsForEdition2015(t *testing.T) {
	tree := map[string]*FileFacts{
		"src/lib.rs": mustParse(t, "src/lib.rs", "mod child;\nmod a;\n"),
		"src/a.rs":   mustParse(t, "src/a.rs", "use child::X;\nuse other::Y;\n"),
	}
	if got := strings.Join(importsForEdition(tree, "2015", "src/lib.rs").production, ","); got != "other" {
		t.Errorf("2015 production = %q, want other", got)
	}
	if got := strings.Join(importsForEdition(tree, "2018", "src/lib.rs").production, ","); got != "child,other" {
		t.Errorf("2018 production = %q, want child,other", got)
	}
}

func TestImportsForModuleScope(t *testing.T) {
	cases := []struct {
		name       string
		files      map[string]string
		production string
		test       string
	}{
		{"nested module is not a root", map[string]string{"src/lib.rs": "mod outer { mod child; }\nuse child::Thing;\n"}, "child", ""},
		{"use beside nested module", map[string]string{"src/lib.rs": "mod outer { mod child; use child::Thing; }\n"}, "", ""},
		{"top-level module", map[string]string{"src/lib.rs": "mod child;\nuse child::Thing;\n"}, "", ""},
		{"test module helpers", map[string]string{"src/lib.rs": "#[cfg(test)]\nmod tests { mod helpers; use helpers::X; }\n"}, "", ""},
		{"module from another file", map[string]string{"src/lib.rs": "mod a;\nmod b;\n", "src/a.rs": "use b::X;\n"}, "b", ""},
		{"child of non-root file", map[string]string{"src/lib.rs": "mod a;\n", "src/a.rs": "mod inner;\nuse inner::X;\n"}, "", ""},
		{"extern crate self", map[string]string{"src/lib.rs": "extern crate self as me;\n"}, "", ""},
		{"local enum glob", map[string]string{"src/lib.rs": "enum Kind { A }\nuse Kind::*;\n"}, "", ""},
		{"pub enum in module", map[string]string{"src/lib.rs": "mod m { pub enum Kind { A } use Kind::A; }\n"}, "", ""},
		{"enum scoped to module", map[string]string{"src/lib.rs": "mod m { enum Kind { A } }\nuse Kind::A;\n"}, "Kind", ""},
		{"fn local external use", map[string]string{"src/lib.rs": "fn f() { use serde_json::Value; }\n"}, "serde_json", ""},
		{"test fn local use", map[string]string{"src/lib.rs": "#[test]\nfn t() { use tempfile::tempdir; }\n"}, "", "tempfile"},
		{"fn local use of module", map[string]string{"src/lib.rs": "mod util;\nfn f() { use util::X; }\n"}, "", ""},
		{"fn local enum glob", map[string]string{"src/lib.rs": "fn f() { enum Kind { A } use Kind::*; }\n"}, "", ""},
		{"chained use alias", map[string]string{"src/lib.rs": "use foo as bar;\nuse bar::X;\n"}, "foo", ""},
	}
	for _, c := range cases {
		tree := map[string]*FileFacts{}
		for file, src := range c.files {
			tree[file] = mustParse(t, file, src)
		}
		got := importsFor(tree)
		if p := strings.Join(got.production, ","); p != c.production {
			t.Errorf("%s: production = %q, want %q", c.name, p, c.production)
		}
		if p := strings.Join(got.test, ","); p != c.test {
			t.Errorf("%s: test = %q, want %q", c.name, p, c.test)
		}
	}
}

func TestOwnershipAndExistingClaimsFail(t *testing.T) {
	tree := map[string]*FileFacts{"src/shared.rs": {}}
	owners := map[string]sourceOwner{}
	if err := claimSources(owners, "first", false, tree); err != nil {
		t.Fatal(err)
	}
	if err := claimSources(owners, "second", false, tree); err == nil || !strings.Contains(err.Error(), "both first and second") {
		t.Errorf("duplicate ownership = %v", err)
	}
	for _, tc := range []struct {
		first, second bool
		fail          bool
	}{
		{first: true, second: true},
		{first: true, second: false, fail: true},
		{first: false, second: true, fail: true},
	} {
		owners := map[string]sourceOwner{}
		if err := claimSources(owners, "first", tc.first, tree); err != nil {
			t.Fatal(err)
		}
		err := claimSources(owners, "second", tc.second, tree)
		if tc.fail && (err == nil || !strings.Contains(err.Error(), "both first and second")) {
			t.Errorf("shared=%v,%v ownership = %v", tc.first, tc.second, err)
		}
		if !tc.fail && err != nil {
			t.Errorf("shared=%v,%v ownership = %v", tc.first, tc.second, err)
		}
		if got := owners["src/shared.rs"].name; got != "first" {
			t.Errorf("shared=%v,%v owner = %q, want first", tc.first, tc.second, got)
		}
	}
	f := rule.EmptyFile("BUILD.bazel", "pkg")
	f.Rules = append(f.Rules, rule.NewRule("filegroup", "collision"))
	generated := []*rule.Rule{rule.NewRule(libraryKind, "collision")}
	if err := checkExistingClaims(f, nil, generated); err == nil || !strings.Contains(err.Error(), "existing filegroup") {
		t.Errorf("existing collision = %v", err)
	}
	if err := checkExistingClaims(nil, []*rule.Rule{rule.NewRule("py_library", "collision")}, generated); err == nil {
		t.Error("other-language collision accepted")
	}
	if err := checkExistingClaims(nil, nil, generated); err != nil {
		t.Errorf("unique generated target rejected: %v", err)
	}
}

func resolverConfig(t *testing.T, directives []rule.Directive) *config.Config {
	t.Helper()
	cfg := config.New()
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", cfg)
	if err := resolver.CheckFlags(nil, cfg); err != nil {
		t.Fatal(err)
	}
	resolver.Configure(cfg, "app", &rule.File{Directives: directives})
	return cfg
}

func resolverIndex(lang *rustLang, entries ...struct {
	pkg  string
	name string
}) *resolve.RuleIndex {
	index := resolve.NewRuleIndex(func(r *rule.Rule, _ string) resolve.Resolver {
		if _, ok := rustKinds[r.Kind()]; ok {
			return lang
		}
		return nil
	})
	for _, entry := range entries {
		r := rule.NewRule(libraryKind, entry.name)
		r.SetAttr("crate_name", entry.name)
		index.AddRule(config.New(), r, rule.EmptyFile(filepath.Join(entry.pkg, "BUILD.bazel"), entry.pkg))
	}
	index.Finish()
	return index
}

func TestResolveBranches(t *testing.T) {
	l := &rustLang{}
	index := resolverIndex(l,
		struct{ pkg, name string }{"lib/b", "b"},
		struct{ pkg, name string }{"lib/a", "a"},
	)
	cfg := resolverConfig(t, nil)
	cfg.Exts[languageName] = &rustConfig{}
	r := rule.NewRule(binaryKind, "app")
	l.Resolve(cfg, index, nil, r, targetImports{production: []string{"b", "a"}}, label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//lib/a,//lib/b" {
		t.Errorf("resolved deps = %q", got)
	}

	self := rule.NewRule(libraryKind, "a")
	l.Resolve(cfg, index, nil, self, targetImports{production: []string{"a"}}, label.New("", "lib/a", "a"))
	if self.Attr("deps") != nil {
		t.Errorf("self dependency was emitted: %v", self.AttrStrings("deps"))
	}

	ignored := &common.IgnoreEntry{Value: "missing"}
	cfg.Exts[languageName] = &rustConfig{IgnoreConfig: common.IgnoreConfig{Ignores: []*common.IgnoreEntry{ignored}}}
	l.Resolve(cfg, index, nil, rule.NewRule(binaryKind, "ignored"), targetImports{production: []string{"missing"}}, label.New("", "app", "ignored"))
	if !ignored.Used() {
		t.Error("exact ignore was not consumed")
	}

	l.Resolve(cfg, index, nil, rule.NewRule(binaryKind, "unknown"), targetImports{production: []string{"unknown"}}, label.New("", "app", "unknown"))
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "unresolved import") {
		t.Errorf("unresolved errors = %v", l.errors)
	}
	l.Resolve(cfg, index, nil, r, nil, label.New("", "app", "app"))
}

func TestResolveExactMappingForTestImports(t *testing.T) {
	from := label.New("", "app/tests", "app_test")
	imports := targetImports{test: []string{"serde", "serde_json"}}

	l := &rustLang{}
	cfg := resolverConfig(t, []rule.Directive{
		{Key: "resolve", Value: "rust serde @crates//:serde"},
		{Key: "resolve", Value: "rust serde_json @crates//:serde_json"},
	})
	cfg.Exts[languageName] = &rustConfig{}
	r := rule.NewRule(testKind, "app_test")
	r.SetAttr("deps", []string{"//cli/cli:dx_cli"})
	l.Resolve(cfg, resolverIndex(l), nil, r, imports, from)
	if len(l.errors) != 0 {
		t.Fatalf("mapped test imports failed: %v", l.errors)
	}
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//cli/cli:dx_cli,@crates//:serde,@crates//:serde_json" {
		t.Errorf("resolved deps = %q", got)
	}

	unmapped := &rustLang{}
	bare := resolverConfig(t, nil)
	bare.Exts[languageName] = &rustConfig{}
	unmapped.Resolve(bare, resolverIndex(unmapped), nil, rule.NewRule(testKind, "app_test"), targetImports{test: []string{"serde"}}, from)
	if len(unmapped.errors) != 1 || !strings.Contains(unmapped.errors[0], `unresolved import "serde"`) {
		t.Errorf("unmapped errors = %v", unmapped.errors)
	}
}

func TestResolveRenamedImports(t *testing.T) {
	l := &rustLang{}
	index := resolverIndex(l, struct{ pkg, name string }{"crates/api", "api"})
	cfg := resolverConfig(t, nil)
	cfg.Exts[languageName] = &rustConfig{}
	from := label.New("", "app", "app")
	renames := map[string]string{"apiv": "api"}

	r := rule.NewRule(libraryKind, "app")
	r.SetAttr("aliases", cargoCall("aliases", "app", false))
	l.Resolve(cfg, index, nil, r, targetImports{production: []string{"apiv"}, renames: renames}, from)
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//crates/api" {
		t.Errorf("deps = %q", got)
	}
	text := bzl.FormatString(r.Attr("aliases"))
	if !strings.Contains(text, `"//crates/api": "apiv"`) || !strings.Contains(text, "|") {
		t.Errorf("aliases = %s", text)
	}

	bare := rule.NewRule(libraryKind, "bare")
	l.Resolve(cfg, index, nil, bare, targetImports{mirrorPaths: []string{"apiv"}, renames: renames}, from)
	if got := bzl.FormatString(bare.Attr("aliases")); !strings.Contains(got, `"//crates/api": "apiv"`) || strings.Contains(got, "|") {
		t.Errorf("mirror aliases = %s", got)
	}

	plain := rule.NewRule(libraryKind, "plain")
	l.Resolve(cfg, index, nil, plain, targetImports{production: []string{"api"}}, from)
	if plain.Attr("aliases") != nil {
		t.Errorf("unrenamed import set aliases")
	}
	if len(l.errors) != 0 {
		t.Fatalf("errors = %v", l.errors)
	}

	// Conflicting names for one label.
	conflict := rule.NewRule(libraryKind, "conflict")
	l.Resolve(cfg, index, nil, conflict, targetImports{production: []string{"one", "two"}, renames: map[string]string{"one": "api", "two": "api"}}, from)
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "conflicting aliases") {
		t.Errorf("conflict errors = %v", l.errors)
	}

	// Unresolved renamed import.
	l.errors = nil
	missing := rule.NewRule(libraryKind, "missing")
	l.Resolve(cfg, index, nil, missing, targetImports{production: []string{"gone"}, renames: map[string]string{"gone": "nowhere"}}, from)
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], `renamed package crate "nowhere"`) {
		t.Errorf("unresolved errors = %v", l.errors)
	}

	// An override on the import name wins and adds no alias.
	l.errors = nil
	ocfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "rust apiv //other:x"}})
	ocfg.Exts[languageName] = &rustConfig{}
	over := rule.NewRule(libraryKind, "over")
	l.Resolve(ocfg, index, nil, over, targetImports{production: []string{"apiv"}, renames: renames}, from)
	if got := strings.Join(over.AttrStrings("deps"), ","); got != "//other:x" || over.Attr("aliases") != nil || len(l.errors) != 0 {
		t.Errorf("override deps=%q aliases=%v errors=%v", got, over.Attr("aliases"), l.errors)
	}
}

func TestAliasesUnionExpr(t *testing.T) {
	e := aliasesUnionExpr{extra: map[string]string{"//b": "y", "//a": "x"}}
	if got := bzl.FormatString(e.BzlExpr()); strings.Index(got, `"//a": "x"`) < 0 || strings.Index(got, `"//a": "x"`) > strings.Index(got, `"//b": "y"`) {
		t.Errorf("nil base = %s", got)
	}
	if first, second := bzl.FormatString(e.Merge(nil)), bzl.FormatString(e.Merge(e.BzlExpr())); first != second {
		t.Errorf("Merge not idempotent: %s vs %s", first, second)
	}
}

func TestLocalCargoImportsRenames(t *testing.T) {
	m := &cargoManifest{
		normalDeps: map[string]cargoDependency{"apiv": {label: "api", depPath: "../api", renamed: true, crate: "api_core"}, "json": {external: true, label: "serde_json", renamed: true}, "same": {label: "same", depPath: "../s", crate: "same"}},
		devDeps:    map[string]cargoDependency{"devv": {label: "d", depPath: "../d", renamed: true, crate: "d"}, "devr": {label: "d2", depPath: "../d2", renamed: true, crate: "d2x"}},
		buildDeps:  map[string]cargoDependency{"bld": {label: "b", depPath: "../b", renamed: true, crate: "b2"}},
	}
	cfg := config.New()
	got := localCargoImports(cfg, m, targetImports{production: []string{"apiv", "json", "devr"}, test: []string{"devr"}}, true)
	if got.renames["apiv"] != "api_core" || got.renames["devr"] != "d2x" || got.renames["same"] != "" || got.renames["json"] != "" {
		t.Errorf("renames = %v", got.renames)
	}
	ex := localCargoExampleImports(cfg, m, targetImports{production: []string{"apiv", "devr"}})
	if ex.renames["apiv"] != "api_core" || ex.renames["devr"] != "d2x" {
		t.Errorf("example renames = %v", ex.renames)
	}
	build := localCargoBuildImports(cfg, m, targetImports{production: []string{"bld"}, test: []string{"bld"}})
	if build.renames["bld"] != "b2" {
		t.Errorf("build renames = %v", build.renames)
	}
}

func TestResolveRenamedProviders(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "api/Cargo.toml", "[package]\nname = \"api\"\n[lib]\nname = \"api_core\"\n")
	writeFixture(t, root, "app/Cargo.toml", "[package]\nname = \"app\"\n")
	m, err := parseCargoManifest("app/Cargo.toml", []byte("[package]\nname = \"app\"\n[dependencies]\napiv = { path = \"../api\", package = \"api\" }\nmissing = { path = \"../nope\", package = \"nope-pkg\" }\nescape = { path = \"../../..\", package = \"esc\" }\nplain = { path = \"../plain\" }\n"))
	if err != nil {
		t.Fatal(err)
	}
	l := &rustLang{}
	l.resolveRenamedProviders(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Rel: "app"}, m)
	for key, want := range map[string]string{"apiv": "api_core", "missing": "nope_pkg", "escape": "esc", "plain": ""} {
		if got := m.normalDeps[key].crate; got != want {
			t.Errorf("%s crate = %q, want %q", key, got, want)
		}
	}
}
