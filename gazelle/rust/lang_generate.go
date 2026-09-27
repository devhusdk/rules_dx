package rust

import (
	"fmt"
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/language"
	bzl "github.com/bazelbuild/buildtools/build"
	"github.com/ralvik/rules_dx/gazelle/common"
)

func (l *rustLang) generateCargo(args language.GenerateArgs, files []string, plan *nativePlan) language.GenerateResult {
	manifestPath := path.Join(args.Rel, "Cargo.toml")
	content, err := os.ReadFile(filepath.Join(args.Dir, "Cargo.toml"))
	if err != nil {
		l.fail("rust: %s: %v", manifestPath, err)
		return language.GenerateResult{}
	}
	manifest, err := parseCargoManifest(manifestPath, content)
	if err != nil {
		l.fail("%v", err)
		return language.GenerateResult{}
	}
	if manifest.virtual {
		return l.attachNative(args, language.GenerateResult{}, plan)
	}
	existsSet := make(map[string]bool, len(files))
	for _, name := range files {
		existsSet[name] = true
	}
	if err := manifest.withImplicitTargets(existsSet, args.Rel); err != nil {
		l.fail("rust: %s: %v", manifestPath, err)
		return language.GenerateResult{}
	}
	if err := checkPathDepVersions(args.Dir, manifestPath, manifest); err != nil {
		l.fail("rust: %v", err)
		return language.GenerateResult{}
	}
	read := func(name string) ([]byte, error) {
		return os.ReadFile(filepath.Join(args.Config.RepoRoot, filepath.FromSlash(name)))
	}
	var result language.GenerateResult
	owners := make(map[string]string)
	publicLibs := shouldSetVisibility(args)
	for _, target := range manifest.targets {
		root := path.Join(args.Rel, target.path)
		if !existsSet[root] {
			if (target.kind == libraryKind && target.path == "src/lib.rs") || (target.kind == binaryKind && target.path == "src/main.rs") {
				continue
			}
			l.fail("rust: %s: Cargo target %s(%s) root %s does not exist", manifestPath, target.kind, target.name, target.path)
			continue
		}
		tree, loadErr := LoadCrate(root, read, func(name string) bool { return existsSet[name] })
		if loadErr != nil {
			l.fail("%v", loadErr)
			continue
		}
		if err := claimSources(owners, target.name, tree); err != nil {
			l.fail("%v", err)
			continue
		}
		r := crateRule(dxKindFor(target), target.name, target.crate(), root, tree, args.Rel)
		r.SetAttr("edition", manifest.edition)
		if isLibraryKind(r.Kind()) && publicLibs {
			r.SetAttr("visibility", []string{"//visibility:public"})
		}
		imports := importsFor(tree)
		if target.kind == exampleKind || target.kind == benchKind {
			if err := validateExampleImports(args.Config, manifest, imports); err != nil {
				l.fail("rust: %s: target %s: %v", manifestPath, target.name, err)
				continue
			}
		} else if err := validateCargoImports(args.Config, manifest, target.kind, imports); err != nil {
			l.fail("rust: %s: target %s: %v", manifestPath, target.name, err)
			continue
		}
		includeDev := target.kind == testKind || target.kind == exampleKind || target.kind == benchKind
		setCargoAttrs(r, args.Rel, manifest, imports, includeDev)
		var resultImports targetImports
		if target.kind == exampleKind || target.kind == benchKind {
			resultImports = localCargoExampleImports(args.Config, manifest, imports)
		} else {
			resultImports = localCargoImports(args.Config, manifest, imports, includeDev)
		}
		if dxKindFor(target) == binaryKind || target.kind == testKind {
			resultImports.siblingLib = siblingLibName(manifest, target)
		}
		if target.kind == testKind && !target.harness {
			r.SetAttr("use_libtest_harness", false)
		}
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, resultImports)
		if wantsUnitTest(target, tree) {
			var testName string
			switch target.kind {
			case exampleKind:
				testName = ExampleTestName(target.name)
			case binaryKind:
				testName = UnitTestName(target.name + "_bin")
			default:
				testName = UnitTestName(target.crate())
			}
			t := rule.NewRule(testKind, testName)
			t.SetAttr("crate", ":"+target.name)
			setCargoAttrs(t, args.Rel, manifest, targetImports{test: imports.test}, true)
			result.Gen = append(result.Gen, t)
			wrapperImports := localCargoImports(args.Config, manifest, targetImports{test: imports.test}, true)
			wrapperImports.mirrorPaths = nil
			result.Imports = append(result.Imports, wrapperImports)
		}
	}
	if manifest.build != nil && !manifest.build.disabled {
		l.emitBuildScript(args, manifestPath, manifest, existsSet, read, owners, &result)
	}
	if len(l.errors) > 0 {
		return language.GenerateResult{}
	}
	return l.attachNative(args, result, plan)
}

func dxKindFor(target cargoTarget) string {
	switch target.kind {
	case exampleKind, benchKind:
		return binaryKind
	case libraryKind:
		switch target.flavor {
		case "proc-macro":
			return procMacroKind
		case "cdylib":
			return sharedKind
		case "staticlib":
			return staticKind
		}
	}
	return target.kind
}

func wantsUnitTest(target cargoTarget, tree map[string]*FileFacts) bool {
	if target.kind == testKind || target.kind == benchKind {
		return false
	}
	if target.kind == exampleKind && !target.exampleTest {
		return false
	}
	return hasUnitTests(tree)
}

func (l *rustLang) emitBuildScript(args language.GenerateArgs, manifestPath string, manifest *cargoManifest, existsSet map[string]bool, read func(string) ([]byte, error), owners map[string]string, result *language.GenerateResult) {
	scriptName, err := BuildScriptName(manifest.packageName)
	if err != nil {
		l.fail("rust: %s: %v", manifestPath, err)
		return
	}
	root := path.Join(args.Rel, manifest.build.path)
	if !existsSet[root] {
		l.fail("rust: %s: build script %s does not exist", manifestPath, manifest.build.path)
		return
	}
	tree, loadErr := LoadCrate(root, read, func(name string) bool { return existsSet[name] })
	if loadErr != nil {
		l.fail("%v", loadErr)
		return
	}
	if err := claimSources(owners, scriptName, tree); err != nil {
		l.fail("%v", err)
		return
	}
	imports := importsFor(tree)
	if err := validateBuildImports(args.Config, manifest, imports); err != nil {
		l.fail("rust: %s: build script: %v", manifestPath, err)
		return
	}
	r := rule.NewRule(scriptKind, scriptName)
	rel := manifest.build.path
	r.SetAttr("srcs", []string{rel})
	r.SetAttr("crate_root", rel)
	r.SetAttr("crate_name", strings.ReplaceAll(scriptName, "-", "_"))
	r.SetAttr("edition", manifest.edition)
	if manifest.version != "" {
		r.SetAttr("version", manifest.version)
	}
	r.SetAttr("pkg_name", manifest.packageName)
	r.SetAttr("crate_features", []string{})
	r.SetAttr("emit_warnings", true)
	r.SetAttr("use_cc_toolchain", 1)
	r.SetAttr("use_default_shell_env", 0)
	setScriptAttrs(r, args.Rel, manifest)
	for i, raw := range result.Imports {
		if current, ok := raw.(targetImports); ok {
			current.scriptDep = scriptName
			result.Imports[i] = current
		}
	}
	result.Gen = append(result.Gen, r)
	result.Imports = append(result.Imports, localCargoBuildImports(args.Config, manifest, imports))
}

func claimSources(owners map[string]string, owner string, tree map[string]*FileFacts) error {
	for source := range tree {
		if previous, ok := owners[source]; ok {
			return fmt.Errorf("rust: source %s is owned by both %s and %s", source, previous, owner)
		}
		owners[source] = owner
	}
	return nil
}

func checkExistingClaims(file *rule.File, other, generated []*rule.Rule) error {
	claims := make(map[string]string)
	if file != nil {
		for _, existing := range file.Rules {
			claims[existing.Name()] = existing.Kind()
		}
	}
	for _, existing := range other {
		claims[existing.Name()] = existing.Kind()
	}
	for _, proposed := range generated {
		if kind, ok := claims[proposed.Name()]; ok && kind != proposed.Kind() {
			if kind == dxCrateKind && proposed.Kind() == libraryKind {
				continue
			}
			return fmt.Errorf("rust: target name %q is claimed by generated %s and existing %s", proposed.Name(), proposed.Kind(), kind)
		}
		claims[proposed.Name()] = proposed.Kind()
	}
	return nil
}

type crateDepsCall struct {
	names       []string
	packageName string
}

func (c crateDepsCall) BzlExpr() bzl.Expr {
	return &bzl.CallExpr{X: &bzl.Ident{Name: "crate_deps"}, List: []bzl.Expr{
		rule.ExprFromValue(c.names),
		&bzl.AssignExpr{LHS: &bzl.Ident{Name: "package_name"}, Op: "=", RHS: &bzl.StringExpr{Value: c.packageName}},
	}}
}

func (c crateDepsCall) Merge(other bzl.Expr) bzl.Expr {
	managed := make(map[string]bool, len(c.names))
	for _, name := range c.names {
		managed[name] = true
	}
	if hand := unmanagedDepsLabels(other, managed); len(hand) > 0 {
		return depsConcatExpr{base: c.BzlExpr(), extra: hand}.BzlExpr()
	}
	return c.BzlExpr()
}

type depsConcatExpr struct {
	base  bzl.Expr
	extra []string
}

func (c depsConcatExpr) BzlExpr() bzl.Expr {
	extra := make([]bzl.Expr, len(c.extra))
	for i, dep := range c.extra {
		extra[i] = &bzl.StringExpr{Value: dep}
	}
	return &bzl.BinaryExpr{X: c.base, Op: "+", Y: &bzl.ListExpr{List: extra}}
}

func (c depsConcatExpr) Merge(other bzl.Expr) bzl.Expr {
	managed := make(map[string]bool, len(c.extra))
	for _, dep := range c.extra {
		managed[dep] = true
	}
	for _, name := range cargoCallNames(c.base) {
		managed[name] = true
	}
	return depsConcatExpr{base: c.base, extra: unionStrings(c.extra, unmanagedDepsLabels(other, managed))}.BzlExpr()
}

func cargoCallNames(base bzl.Expr) []string {
	call, ok := base.(*bzl.CallExpr)
	if !ok {
		return nil
	}
	if ident, ok := call.X.(*bzl.Ident); !ok || ident.Name != "crate_deps" {
		return nil
	}
	if len(call.List) == 0 {
		return nil
	}
	list, ok := call.List[0].(*bzl.ListExpr)
	if !ok {
		return nil
	}
	var names []string
	for _, item := range list.List {
		if s, ok := item.(*bzl.StringExpr); ok {
			names = append(names, s.Value)
		}
	}
	return names
}

func unmanagedDepsLabels(other bzl.Expr, managed map[string]bool) []string {
	var out []string
	var walk func(e bzl.Expr)
	walk = func(e bzl.Expr) {
		switch e := e.(type) {
		case *bzl.ListExpr:
			for _, item := range e.List {
				if s, ok := item.(*bzl.StringExpr); ok && !managed[s.Value] {
					out = append(out, s.Value)
				}
			}
		case *bzl.BinaryExpr:
			if e.Op == "+" {
				walk(e.X)
				walk(e.Y)
			}
		}
	}
	if other != nil {
		walk(other)
	}
	sort.Strings(out)
	return out
}

type cargoCallExpr struct {
	name         string
	packageName  string
	includeDev   bool
	includeBuild bool
}

func cargoCall(name, packageName string, includeDev bool) cargoCallExpr {
	return cargoCallExpr{name: name, packageName: packageName, includeDev: includeDev}
}

func cargoBuildCall(packageName string) cargoCallExpr {
	return cargoCallExpr{name: "aliases", packageName: packageName, includeBuild: true}
}

func (c cargoCallExpr) BzlExpr() bzl.Expr {
	var args []bzl.Expr
	if !c.includeBuild {
		args = append(args, &bzl.AssignExpr{LHS: &bzl.Ident{Name: "normal"}, Op: "=", RHS: &bzl.Ident{Name: "True"}})
	}
	args = append(args, &bzl.AssignExpr{LHS: &bzl.Ident{Name: "package_name"}, Op: "=", RHS: &bzl.StringExpr{Value: c.packageName}})
	if c.includeDev {
		args = append(args, &bzl.AssignExpr{LHS: &bzl.Ident{Name: "normal_dev"}, Op: "=", RHS: &bzl.Ident{Name: "True"}})
	}
	if c.includeBuild {
		args = append(args, &bzl.AssignExpr{LHS: &bzl.Ident{Name: "build"}, Op: "=", RHS: &bzl.Ident{Name: "True"}})
	}
	return &bzl.CallExpr{X: &bzl.Ident{Name: c.name}, List: args}
}

func (c cargoCallExpr) Merge(bzl.Expr) bzl.Expr { return c.BzlExpr() }

func sourceFiles(dir, rel string) ([]string, error) {
	var files []string
	for _, root := range []string{dir} {
		err := filepath.WalkDir(root, func(name string, entry os.DirEntry, err error) error {
			if err != nil {
				return err
			}
			if entry.IsDir() && name != root {
				if strings.HasPrefix(entry.Name(), ".") || entry.Name() == "bazel-bin" || entry.Name() == "bazel-out" {
					return filepath.SkipDir
				}
				if _, statErr := os.Stat(filepath.Join(name, "BUILD.bazel")); statErr == nil {
					return filepath.SkipDir
				}
				if _, statErr := os.Stat(filepath.Join(name, "BUILD")); statErr == nil {
					return filepath.SkipDir
				}
			}
			if !entry.IsDir() && strings.HasSuffix(entry.Name(), ".rs") {
				local, relErr := filepath.Rel(dir, name)
				if relErr != nil {
					return relErr
				}
				files = append(files, path.Join(rel, filepath.ToSlash(local)))
			}
			return nil
		})
		if err != nil {
			return nil, err
		}
	}
	sort.Strings(files)
	return files, nil
}

func hasUnitTests(tree map[string]*FileFacts) bool {
	for _, facts := range tree {
		if facts.HasTestAttr || facts.HasCfgTest || facts.InnerCfgTest {
			return true
		}
	}
	return false
}

func crateRule(kind, name, crateName, root string, tree map[string]*FileFacts, pkg string) *rule.Rule {
	r := rule.NewRule(kind, name)
	srcs := make([]string, 0, len(tree))
	for source := range tree {
		srcs = append(srcs, strings.TrimPrefix(source, strings.TrimSuffix(pkg, "/")+"/"))
	}
	sort.Strings(srcs)
	r.SetAttr("srcs", srcs)
	r.SetAttr("crate_root", strings.TrimPrefix(root, strings.TrimSuffix(pkg, "/")+"/"))
	r.SetAttr("crate_name", crateName)
	return r
}

func importsFor(tree map[string]*FileFacts) targetImports {
	sets := [2]map[string]bool{{}, {}}
	localModules := make(map[string]bool)
	for _, facts := range tree {
		for _, module := range facts.Modules {
			localModules[module.Name] = true
		}
	}
	for _, facts := range tree {
		for _, use := range facts.Uses {
			addImport(sets, localModules, use.Path, use.CfgTest)
		}
		for _, ext := range facts.Externs {
			name := ext.Name
			if ext.As != "" {
				name = ext.As
			}
			addImport(sets, localModules, name, ext.CfgTest)
		}
	}
	result := targetImports{}
	for imp := range sets[0] {
		result.production = append(result.production, imp)
	}
	for imp := range sets[1] {
		result.test = append(result.test, imp)
	}
	sort.Strings(result.production)
	sort.Strings(result.test)
	return result
}

func addImport(sets [2]map[string]bool, localModules map[string]bool, raw string, test bool) {
	root := strings.TrimPrefix(strings.Split(raw, "::")[0], "::")
	if root == "" || localModules[root] || root == "crate" || root == "self" || root == "super" || root == "std" || root == "core" || root == "alloc" {
		return
	}
	index := 0
	if test {
		index = 1
	}
	sets[index][root] = true
}

func isFixturePath(rel string) bool { return common.IsFixturePath(rel) }

func mergeStale(file *rule.File, result language.GenerateResult) language.GenerateResult {
	desired := make(map[string]bool, len(result.Gen))
	for _, r := range result.Gen {
		desired[r.Kind()+"\x00"+r.Name()] = true
	}
	stale := staleRules(file, desired)
	result.Empty = append(result.Empty, stale.Empty...)
	return result
}

func staleRules(file *rule.File, desired map[string]bool) language.GenerateResult {
	var result language.GenerateResult
	if file == nil {
		return result
	}
	for _, existing := range file.Rules {
		if isNativeConfigKind(existing.Kind()) {
			continue
		}
		if _, owned := rustKinds[existing.Kind()]; !owned || desired[existing.Kind()+"\x00"+existing.Name()] {
			continue
		}
		empty := rule.NewRule(existing.Kind(), existing.Name())
		result.Empty = append(result.Empty, empty)
	}
	return result
}

func siblingLibName(manifest *cargoManifest, bin cargoTarget) string {
	for _, target := range manifest.targets {
		if target.kind == libraryKind && target.flavor == "" {
			return target.name
		}
	}
	for _, target := range manifest.targets {
		if target.kind == libraryKind {
			return target.name
		}
	}
	return ""
}
