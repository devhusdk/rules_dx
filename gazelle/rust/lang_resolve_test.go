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
	ignored := &ignoreEntry{value: "b"}
	cfg.Exts[languageName] = &rustConfig{ignores: []*ignoreEntry{ignored}}
	skipped := rule.NewRule(binaryKind, "skipped")
	l.Resolve(cfg, index, nil, skipped, targetImports{mirrorPaths: []string{"b"}}, label.New("", "app", "skipped"))
	if skipped.Attr("deps") != nil || !ignored.used {
		t.Errorf("ignored mirror = deps:%v used:%v", skipped.Attr("deps"), ignored.used)
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
	local := localCargoImports(cfg, manifest, imports, true)
	if strings.Join(local.production, ",") != "local" || strings.Join(local.test, ",") != "local_dev" {
		t.Errorf("local imports = %+v", local)
	}
	r := rule.NewRule(libraryKind, "app")
	setCargoAttrs(r, "pkg/app", manifest, imports, true)
	if r.Attr("deps") == nil || r.Attr("aliases") == nil {
		t.Errorf("Cargo expressions missing")
	}
	call := cargoCall("aliases", "pkg/app", true)
	if call.Merge(nil) == nil || (crateDepsCall{names: []string{"normal"}, packageName: "pkg/app"}).Merge(nil) == nil {
		t.Error("generated expressions did not merge")
	}
	withoutDev := localCargoImports(cfg, manifest, imports, false)
	if len(withoutDev.test) != 0 {
		t.Errorf("production local imports include dev: %+v", withoutDev)
	}
	empty := rule.NewRule(libraryKind, "empty")
	setCargoAttrs(empty, "pkg/app", manifest, targetImports{}, false)
	if empty.Attr("deps") == nil || empty.Attr("aliases") == nil {
		t.Errorf("all-declared Cargo attrs = deps:%v aliases:%v", empty.Attr("deps"), empty.Attr("aliases"))
	}
	dashedManifest := &cargoManifest{
		packageName: "app",
		normalDeps:  map[string]cargoDependency{"quick_xml": {external: true, label: "quick-xml"}, "serde_json": {external: true, label: "serde_json"}},
		devDeps:     map[string]cargoDependency{},
	}
	dashed := rule.NewRule(libraryKind, "dashed")
	setCargoAttrs(dashed, "pkg/app", dashedManifest, targetImports{}, false)
	depsExpr := dashed.Attr("deps")
	depsCall, ok := depsExpr.(*bzl.CallExpr)
	if !ok {
		t.Fatalf("dashed deps attr = %T, want crate_deps call", depsExpr)
	} else {
		var got []string
		if len(depsCall.List) > 0 {
			if list, ok := depsCall.List[0].(*bzl.ListExpr); ok {
				for _, item := range list.List {
					if s, ok := item.(*bzl.StringExpr); ok {
						got = append(got, s.Value)
					}
				}
			}
		}
		if strings.Join(got, ",") != "quick-xml,serde_json" {
			t.Errorf("dashed externals = %q, want quick-xml,serde_json", strings.Join(got, ","))
		}
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
	c.Exts[languageName] = &rustConfig{ignores: []*ignoreEntry{{value: "mapped"}}}
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
	called.SetAttr("deps", crateDepsCall{names: []string{"serde_json"}, packageName: "pkg"})
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
		base:  crateDepsCall{names: []string{"blake3", "serde", "serde_json"}, packageName: "dx/dx_env"}.BzlExpr(),
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
		base:  crateDepsCall{names: []string{"serde_json"}, packageName: "dx/dx_a"}.BzlExpr(),
		extra: []string{":dx_a"},
	}
	merged := src.Merge(crateDepsFileExpr([]string{"serde_json"}, "dx/dx_a", ":dx_a", ":old_lib"))
	if got := tailStrings(t, merged); strings.Join(got, "\x00") != ":dx_a\x00:old_lib" {
		t.Errorf("merged tail = %q, want stale :old_lib preserved", got)
	}
}

func TestCrateDepsCallMerge(t *testing.T) {
	call := crateDepsCall{names: []string{"serde_json"}, packageName: "dx/dx_a"}
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
	setCargoAttrs(r, "pkg/app", manifest, targetImports{}, false)
	call, ok := r.Attr("deps").(*bzl.CallExpr)
	if !ok || len(call.List) != 2 {
		t.Fatalf("deps attr = %v, want crate_deps(names, package_name)", r.Attr("deps"))
	}
	if names, ok := call.List[0].(*bzl.ListExpr); !ok || len(names.List) != 0 {
		t.Fatalf("crate_deps names = %v, want none", call.List[0])
	}
	merged := crateDepsCall{packageName: "pkg/app"}.Merge(strListExpr("//cli/digest:dx_digest", "//cli/process:dx_process"))
	got := strings.Join(tailStrings(t, merged), "\x00")
	if got != "//cli/digest:dx_digest\x00//cli/process:dx_process" {
		t.Errorf("merged tail = %q, want the hand-written labels kept", got)
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
	if strings.Join(aliasedImports.production, ",") != "proto" {
		t.Errorf("aliased extern imports = %+v, want [proto]", aliasedImports)
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

func TestOwnershipAndExistingClaimsFail(t *testing.T) {
	tree := map[string]*FileFacts{"src/shared.rs": {}}
	owners := map[string]string{}
	if err := claimSources(owners, "first", tree); err != nil {
		t.Fatal(err)
	}
	if err := claimSources(owners, "second", tree); err == nil || !strings.Contains(err.Error(), "both first and second") {
		t.Errorf("duplicate ownership = %v", err)
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

	ignored := &ignoreEntry{value: "missing"}
	cfg.Exts[languageName] = &rustConfig{ignores: []*ignoreEntry{ignored}}
	l.Resolve(cfg, index, nil, rule.NewRule(binaryKind, "ignored"), targetImports{production: []string{"missing"}}, label.New("", "app", "ignored"))
	if !ignored.used {
		t.Error("exact ignore was not consumed")
	}

	l.Resolve(cfg, index, nil, rule.NewRule(binaryKind, "unknown"), targetImports{production: []string{"unknown"}}, label.New("", "app", "unknown"))
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "unresolved import") {
		t.Errorf("unresolved errors = %v", l.errors)
	}
	l.Resolve(cfg, index, nil, r, nil, label.New("", "app", "app"))
}
