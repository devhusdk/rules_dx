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
	"github.com/devhusdk/rules_dx/gazelle/common"
)

func (l *rustLang) inheritWorkspace(args language.GenerateArgs, manifestPath string, manifest *cargoManifest) error {
	subject, ok := manifest.inheritsWorkspace()
	if !ok {
		return nil
	}
	rootRel, root, err := l.resolveWorkspaceRoot(args.Config.RepoRoot, args.Rel, manifest)
	if err != nil {
		return err
	}
	if root == nil {
		return fmt.Errorf("rust: %s: %s sets workspace = true but no parent Cargo.toml declares [workspace]", manifestPath, subject)
	}
	err = manifest.inheritWorkspaceDeps(args.Rel, rootRel, root)
	if err == nil {
		err = manifest.inheritWorkspacePackage(rootRel, root)
	}
	if err != nil {
		return fmt.Errorf("rust: %s: %v", manifestPath, err)
	}
	return nil
}

func escapesRepo(rel string) bool {
	return rel == ".." || strings.HasPrefix(rel, "../")
}

func (l *rustLang) resolveWorkspaceRoot(repoRoot, rel string, m *cargoManifest) (string, *cargoManifest, error) {
	lookup := func(dir string) (*cargoManifest, error) {
		if dir == rel {
			return m, nil
		}
		return l.cargoManifestAt(repoRoot, dir)
	}
	if m.workspacePath != "" {
		manifestPath := path.Join(rel, "Cargo.toml")
		rootRel := path.Clean(path.Join(rel, m.workspacePath))
		if rootRel == "." {
			rootRel = ""
		}
		if escapesRepo(rootRel) {
			return "", nil, fmt.Errorf("rust: %s: package.workspace %q leaves the repository", manifestPath, m.workspacePath)
		}
		root, err := lookup(rootRel)
		if err != nil {
			return "", nil, err
		}
		if root == nil || !root.workspaceRoot {
			return "", nil, fmt.Errorf("rust: %s: package.workspace points at %s which declares no [workspace]", manifestPath, path.Join(rootRel, "Cargo.toml"))
		}
		return rootRel, root, nil
	}
	for dir := rel; ; dir = path.Dir(dir) {
		if dir == "." {
			dir = ""
		}
		root, err := lookup(dir)
		if err != nil {
			return "", nil, err
		}
		if root != nil && root.workspaceRoot {
			return dir, root, nil
		}
		if dir == "" {
			return "", nil, nil
		}
	}
}

func (l *rustLang) cargoManifestAt(repoRoot, rel string) (*cargoManifest, error) {
	if m, ok := l.workspaceRoots[rel]; ok {
		return m, nil
	}
	if l.workspaceRoots == nil {
		l.workspaceRoots = make(map[string]*cargoManifest)
	}
	content, err := os.ReadFile(filepath.Join(repoRoot, filepath.FromSlash(rel), "Cargo.toml"))
	if err != nil {
		l.workspaceRoots[rel] = nil
		return nil, nil
	}
	m, err := parseCargoManifest(path.Join(rel, "Cargo.toml"), content)
	if err != nil {
		return nil, err
	}
	l.workspaceRoots[rel] = m
	return m, nil
}

func providerRelOf(rel, depPath string) string {
	providerRel := path.Clean(path.Join(rel, depPath))
	if providerRel == "." {
		providerRel = ""
	}
	return providerRel
}

// resolveRenamedProviders records, for each renamed local path dep, the extern
// crate name of the crate it points at. Only the consumer's maps are written.
func (l *rustLang) resolveRenamedProviders(args language.GenerateArgs, m *cargoManifest) {
	for _, depMap := range []map[string]cargoDependency{m.normalDeps, m.devDeps, m.buildDeps} {
		for key, dep := range depMap {
			if dep.external || !dep.renamed || dep.depPath == "" {
				continue
			}
			providerRel := providerRelOf(args.Rel, dep.depPath)
			if !escapesRepo(providerRel) {
				if provider, _ := l.cargoManifestAt(args.Config.RepoRoot, providerRel); provider != nil {
					dep.crate = provider.libCrateName()
				}
			}
			if dep.crate == "" {
				dep.crate, _ = CrateName(dep.label)
			}
			depMap[key] = dep
		}
	}
}

func (l *rustLang) checkPathDepVersions(args language.GenerateArgs, manifestPath string, m *cargoManifest) error {
	for _, depMap := range []map[string]cargoDependency{m.normalDeps, m.devDeps, m.buildDeps} {
		keys := make([]string, 0, len(depMap))
		for key := range depMap {
			keys = append(keys, key)
		}
		sort.Strings(keys)
		for _, key := range keys {
			dep := depMap[key]
			if dep.external || dep.version == "" || dep.depPath == "" {
				continue
			}
			providerRel := providerRelOf(args.Rel, dep.depPath)
			providerPath := path.Join(providerRel, "Cargo.toml")
			var provider *cargoManifest
			if !escapesRepo(providerRel) {
				provider, _ = l.cargoManifestAt(args.Config.RepoRoot, providerRel)
			}
			if provider == nil {
				return fmt.Errorf("rust: path dependency %q points at %s without a readable Cargo.toml; fix the path", key, providerPath)
			}
			provided := provider.version
			if provider.inheritVersion {
				_, root, err := l.resolveWorkspaceRoot(args.Config.RepoRoot, providerRel, provider)
				if err != nil {
					return err
				}
				provided = ""
				if root != nil {
					provided = root.workspaceVersion
				}
			}
			if provided == "" {
				return fmt.Errorf("rust: path dependency %q at %s declares no [package] version while %s requires %q; version the provider or drop the requirement", key, providerPath, manifestPath, dep.version)
			}
			if !versionReqSatisfied(provided, dep.version) {
				return fmt.Errorf("rust: path dependency %q provides version %s at %s, which does not satisfy %q required by %s", key, provided, providerPath, dep.version, manifestPath)
			}
		}
	}
	return nil
}

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
	if err := l.inheritWorkspace(args, manifestPath, manifest); err != nil {
		l.fail("%v", err)
		return language.GenerateResult{}
	}
	if err := l.checkPathDepVersions(args, manifestPath, manifest); err != nil {
		l.fail("%v", err)
		return language.GenerateResult{}
	}
	l.resolveRenamedProviders(args, manifest)
	read := func(name string) ([]byte, error) {
		return os.ReadFile(filepath.Join(args.Config.RepoRoot, filepath.FromSlash(name)))
	}
	var result language.GenerateResult
	owners := make(map[string]sourceOwner)
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
		includeDev := target.kind == testKind || target.kind == exampleKind || target.kind == benchKind
		if err := claimSources(owners, target.name, includeDev, tree); err != nil {
			l.fail("%v", err)
			continue
		}
		splitTests := excludesTestOnly(target) && hasTestOnly(tree)
		r := crateRule(dxKindFor(target), target.name, target.crate(), root, tree, args.Rel, splitTests)
		r.SetAttr("edition", manifest.edition)
		if isLibraryKind(r.Kind()) && publicLibs {
			r.SetAttr("visibility", []string{"//visibility:public"})
		}
		imports := importsForEdition(tree, manifest.edition, root)
		if target.kind == exampleKind || target.kind == benchKind {
			if err := validateExampleImports(args.Config, manifest, imports); err != nil {
				l.fail("rust: %s: target %s: %v", manifestPath, target.name, err)
				continue
			}
		} else if err := validateCargoImports(args.Config, manifest, target.kind, imports); err != nil {
			l.fail("rust: %s: target %s: %v", manifestPath, target.name, err)
			continue
		}
		setCargoAttrs(r, args.Rel, manifest, includeDev)
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
				testName = common.UnitTestName(target.name)
			case binaryKind:
				testName = common.UnitTestName(common.BinaryName(target.name))
			default:
				testName = common.UnitTestName(target.crate())
			}
			t := unitTestRule(testName, target.name, target.crate(), root, tree, args.Rel, splitTests)
			setCargoAttrs(t, args.Rel, manifest, true)
			var testImports targetImports
			if splitTests {
				t.SetAttr("edition", manifest.edition)
				testImports = localCargoImports(args.Config, manifest, imports, true)
				testImports.siblingLib = resultImports.siblingLib
			} else {
				testImports = localCargoImports(args.Config, manifest, targetImports{test: imports.test}, true)
				testImports.mirrorPaths = nil
			}
			result.Gen = append(result.Gen, t)
			result.Imports = append(result.Imports, testImports)
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

func (l *rustLang) emitBuildScript(args language.GenerateArgs, manifestPath string, manifest *cargoManifest, existsSet map[string]bool, read func(string) ([]byte, error), owners map[string]sourceOwner, result *language.GenerateResult) {
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
	if err := claimSources(owners, scriptName, false, tree); err != nil {
		l.fail("%v", err)
		return
	}
	imports := importsForEdition(tree, manifest.edition, root)
	if err := validateBuildImports(args.Config, manifest, imports); err != nil {
		l.fail("rust: %s: build script: %v", manifestPath, err)
		return
	}
	r := rule.NewRule(scriptKind, scriptName)
	rel := manifest.build.path
	r.SetAttr("srcs", []string{rel})
	r.SetAttr("crate_root", rel)
	r.SetAttr("crate_name", scriptName)
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

type sourceOwner struct {
	name   string
	shared bool
}

func claimSources(owners map[string]sourceOwner, owner string, shared bool, tree map[string]*FileFacts) error {
	for source := range tree {
		if previous, ok := owners[source]; ok {
			if !previous.shared || !shared {
				return fmt.Errorf("rust: source %s is owned by both %s and %s", source, previous.name, owner)
			}
			continue
		}
		owners[source] = sourceOwner{name: owner, shared: shared}
	}
	return nil
}

// rustClaimAliases lets a handwritten dx crate stand in for the generated rust_library of the same name.
var rustClaimAliases = map[string]string{dxCrateKind: libraryKind}

// generatedClaims turns the generated rules of package rel into claimants for common.CheckClaims.
func generatedClaims(rel string, gen []*rule.Rule) []common.Claimant {
	claimants := make([]common.Claimant, 0, len(gen))
	for _, r := range gen {
		claimants = append(claimants, common.Claimant{Name: r.Name(), Source: rel, Kind: r.Kind()})
	}
	return claimants
}

type crateDepsCall struct {
	call     cargoCallExpr
	external bool
}

func (c crateDepsCall) BzlExpr() bzl.Expr {
	if !c.external {
		return &bzl.ListExpr{}
	}
	return c.call.BzlExpr()
}

func (c crateDepsCall) Merge(other bzl.Expr) bzl.Expr {
	hand := unmanagedDepsLabels(other, nil)
	if !c.external {
		return rule.ExprFromValue(hand)
	}
	if len(hand) > 0 {
		return depsConcatExpr{base: c.BzlExpr(), extra: hand}.BzlExpr()
	}
	return c.BzlExpr()
}

// aliasesUnionExpr renders base | {label: name} for the aliases attribute.
type aliasesUnionExpr struct {
	base  bzl.Expr
	extra map[string]string
}

func (c aliasesUnionExpr) BzlExpr() bzl.Expr {
	keys := make([]string, 0, len(c.extra))
	for k := range c.extra {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	dict := &bzl.DictExpr{ForceMultiLine: false}
	for _, k := range keys {
		dict.List = append(dict.List, &bzl.KeyValueExpr{Key: &bzl.StringExpr{Value: k}, Value: &bzl.StringExpr{Value: c.extra[k]}})
	}
	if c.base == nil {
		return dict
	}
	return &bzl.BinaryExpr{X: c.base, Op: "|", Y: dict}
}

func (c aliasesUnionExpr) Merge(bzl.Expr) bzl.Expr { return c.BzlExpr() }

type depsConcatExpr struct {
	base    bzl.Expr
	extra   []string
	android []string
	// platform maps a platform constraint to deps gated behind it.
	platform map[string][]string
}

func (c depsConcatExpr) BzlExpr() bzl.Expr {
	var out bzl.Expr = c.base
	gates := map[string][]string{}
	if len(c.android) > 0 {
		gates[androidPlatformConstraint] = c.android
	}
	for k, v := range c.platform {
		if len(v) > 0 {
			gates[k] = v
		}
	}
	// Keys may be comma-joined constraint sets sharing one dep list.
	if out == nil || len(c.extra) > 0 || len(gates) == 0 {
		extra := make([]bzl.Expr, len(c.extra))
		for i, dep := range c.extra {
			extra[i] = &bzl.StringExpr{Value: dep}
		}
		list := &bzl.ListExpr{List: extra}
		if out == nil {
			out = list
		} else {
			out = &bzl.BinaryExpr{X: out, Op: "+", Y: list}
		}
	}
	if len(gates) == 0 {
		return out
	}
	keys := make([]string, 0, len(gates))
	for k := range gates {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	first := len(c.extra) == 0 && c.base == nil
	for _, k := range keys {
		items := make([]bzl.Expr, len(gates[k]))
		for i, dep := range gates[k] {
			items[i] = &bzl.StringExpr{Value: dep}
		}
		var entries []*bzl.KeyValueExpr
		for _, key := range strings.Split(k, ",") {
			entries = append(entries, &bzl.KeyValueExpr{Key: &bzl.StringExpr{Value: key}, Value: &bzl.ListExpr{List: items}})
		}
		entries = append(entries, &bzl.KeyValueExpr{Key: &bzl.StringExpr{Value: "//conditions:default"}, Value: &bzl.ListExpr{}})
		sel := &bzl.CallExpr{X: &bzl.Ident{Name: "select"}, List: []bzl.Expr{&bzl.DictExpr{List: entries, ForceMultiLine: true}}}
		if first {
			out = sel
			first = false
			continue
		}
		out = &bzl.BinaryExpr{X: out, Op: "+", Y: sel}
	}
	return out
}

func (c depsConcatExpr) Merge(other bzl.Expr) bzl.Expr {
	managed := make(map[string]bool, len(c.extra))
	for _, dep := range c.extra {
		managed[dep] = true
	}
	for _, dep := range c.android {
		managed[dep] = true
	}
	for _, deps := range c.platform {
		for _, dep := range deps {
			managed[dep] = true
		}
	}
	return depsConcatExpr{base: c.base, extra: unionStrings(c.extra, unmanagedDepsLabels(other, managed)), android: c.android, platform: c.platform}.BzlExpr()
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

// unitTestOfAttr is the private attribute naming the crate target a generated
// unit test belongs to, whichever shape the test has.
const unitTestOfAttr = "dx_unit_test_of"

// excludesTestOnly reports whether a Cargo target's production rule leaves
// out TestOnly files. Proc-macro, cdylib, staticlib and example targets keep
// their test modules in srcs and a crate= unit test.
func excludesTestOnly(target cargoTarget) bool {
	switch target.kind {
	case libraryKind:
		return target.flavor == ""
	case binaryKind:
		return true
	}
	return false
}

func hasTestOnly(tree map[string]*FileFacts) bool {
	for _, facts := range tree {
		if facts.TestOnly {
			return true
		}
	}
	return false
}

// crateRule builds a crate rule over tree. With excludeTestOnly, files that
// rustc reads only under cfg(test) are left out of srcs.
func crateRule(kind, name, crateName, root string, tree map[string]*FileFacts, pkg string, excludeTestOnly bool) *rule.Rule {
	r := rule.NewRule(kind, name)
	srcs := make([]string, 0, len(tree))
	for source, facts := range tree {
		if excludeTestOnly && facts.TestOnly {
			continue
		}
		srcs = append(srcs, strings.TrimPrefix(source, strings.TrimSuffix(pkg, "/")+"/"))
	}
	sort.Strings(srcs)
	r.SetAttr("srcs", srcs)
	r.SetAttr("crate_root", strings.TrimPrefix(root, strings.TrimSuffix(pkg, "/")+"/"))
	r.SetAttr("crate_name", crateName)
	return r
}

// unitTestRule builds the unit test of the crate target of. A self-contained
// test compiles every file of tree itself (the crate target left the TestOnly
// files out); otherwise the test wraps the crate target through crate=.
func unitTestRule(name, of, crateName, root string, tree map[string]*FileFacts, pkg string, selfContained bool) *rule.Rule {
	var t *rule.Rule
	if selfContained {
		t = crateRule(testKind, name, crateName, root, tree, pkg, false)
	} else {
		t = rule.NewRule(testKind, name)
		t.SetAttr("crate", ":"+of)
	}
	t.SetPrivateAttr(unitTestOfAttr, of)
	return t
}

func unitTestOf(r *rule.Rule) string {
	if r.Kind() != testKind {
		return ""
	}
	of, _ := r.PrivateAttr(unitTestOfAttr).(string)
	return of
}

func importsFor(tree map[string]*FileFacts) targetImports {
	return importsForEdition(tree, "2021", "")
}

func importsForEdition(tree map[string]*FileFacts, edition, crateRoot string) targetImports {
	sets := [2]map[string]bool{{}, {}}
	localModules := make(map[string]bool)
	for file, facts := range tree {
		for _, module := range facts.Modules {
			localModules[file+"\x00"+module.Scope+"\x00"+module.Name] = true
		}
	}
	for file, facts := range tree {
		for _, local := range facts.Locals {
			localModules[file+"\x00"+local.Scope+"\x00"+local.Name] = true
		}
		for _, ext := range facts.Externs {
			if ext.As != "" {
				localModules[file+"\x00"+ext.Scope+"\x00"+ext.As] = true
			}
		}
	}
	for file, facts := range tree {
		for _, use := range facts.Uses {
			root := useRoot(use.Path)
			if localModules[file+"\x00"+use.Scope+"\x00"+root] {
				continue
			}
			if edition == "2015" && localModules[crateRoot+"\x00\x00"+root] {
				continue
			}
			addImport(sets, root, use.CfgTest)
		}
		for _, ext := range facts.Externs {
			name := ext.Name
			if builtinRoot(name) {
				continue
			}
			addImport(sets, name, ext.CfgTest)
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

func useRoot(raw string) string {
	return strings.Split(strings.TrimPrefix(raw, "::"), "::")[0]
}

func builtinRoot(root string) bool {
	return root == "" || root == "crate" || root == "self" || root == "super" || root == "std" || root == "core" || root == "alloc"
}

func addImport(sets [2]map[string]bool, root string, test bool) {
	if builtinRoot(root) {
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
		if existing.Kind() == corpusKind && !isGeneratedCorpusName(existing.Name()) {
			continue
		}
		if hasHandwrittenAttr(existing) {
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

// hasHandwrittenAttr reports whether r sets attributes the generator never
// writes, so the rule is handwritten and must not be removed as stale.
func hasHandwrittenAttr(r *rule.Rule) bool {
	for _, attr := range []string{"env", "rustc_env", "data", "compile_data", "rustc_flags", "args"} {
		if r.Attr(attr) != nil {
			return true
		}
	}
	return false
}
