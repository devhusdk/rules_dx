package common

import (
	"bytes"
	"fmt"
	"os"
	"path"
	"path/filepath"
	"slices"
	"sort"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
	"github.com/bazelbuild/buildtools/build"
)

type Reporter interface {
	Fail(pkg PackageRef, format string, args ...interface{})
	Failed() bool
}

func LibraryKindInfo() rule.KindInfo {
	return rule.KindInfo{
		MatchAttrs:    []string{"srcs"},
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"srcs": true,
			"deps": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	}
}

func TestKindInfo(extraMergeable ...string) rule.KindInfo {
	info := LibraryKindInfo()
	for _, attr := range extraMergeable {
		info.MergeableAttrs[attr] = true
	}
	return info
}

func fileBuildPath(f *rule.File) string {
	name := "BUILD.bazel"
	if f.Path != "" {
		name = filepath.Base(f.Path)
	}
	return manifestPath(f.Pkg, name)
}

func CheckClaims(file *rule.File, other []*rule.Rule, claimants []Claimant) error {
	byName := make(map[string][]Claimant, len(claimants))
	order := make([]string, 0, len(claimants))
	for _, c := range claimants {
		if _, ok := byName[c.Name]; !ok {
			order = append(order, c.Name)
		}
		byName[c.Name] = append(byName[c.Name], c)
	}
	existing := make(map[string]string)
	fromOther := make(map[string]bool)
	if file != nil {
		for _, r := range file.Rules {
			existing[r.Name()] = r.Kind()
		}
	}
	for _, r := range other {
		existing[r.Name()] = r.Kind()
		fromOther[r.Name()] = true
	}
	for _, name := range order {
		claims := byName[name]
		have, handwritten := existing[name]
		if len(claims) > 1 {
			all := make([]string, 0, len(claims)+1)
			for _, c := range claims {
				all = append(all, c.Source)
			}
			if handwritten {
				all = append(all, "handwritten:"+have+":"+name)
			}
			return &CollisionError{Name: name, Claimants: all}
		}
		if !handwritten {
			continue
		}
		c := claims[0]
		if have != c.Kind && !slices.Contains(c.Kinds, have) {
			buildFile := ""
			if !fromOther[name] && file != nil {
				buildFile = fileBuildPath(file)
			}
			return fmt.Errorf("%s", ClaimError(name, c.Kind, c.Source, have, buildFile))
		}
	}
	return nil
}

func RuleNamesOfKind(file *rule.File, other []*rule.Rule, kind string) map[string]bool {
	names := make(map[string]bool)
	if file != nil {
		for _, r := range file.Rules {
			if r.Kind() == kind {
				names[r.Name()] = true
			}
		}
	}
	for _, r := range other {
		if r.Kind() == kind {
			names[r.Name()] = true
		}
	}
	return names
}

func CustomRunnerTests(file *rule.File, other []*rule.Rule, kind, defaultRunner string) map[string]bool {
	names := make(map[string]bool)
	check := func(r *rule.Rule) {
		if runner := r.AttrString("runner"); r.Kind() == kind && runner != "" && runner != defaultRunner {
			names[r.Name()] = true
		}
	}
	if file != nil {
		for _, r := range file.Rules {
			check(r)
		}
	}
	for _, r := range other {
		check(r)
	}
	return names
}

func MergeStale(file *rule.File, result language.GenerateResult, kinds map[string]rule.KindInfo) language.GenerateResult {
	return MergeStaleOwned(file, result, kinds, nil)
}

func MergeStaleOwned(file *rule.File, result language.GenerateResult, kinds map[string]rule.KindInfo, owns func(*rule.Rule) bool) language.GenerateResult {
	desired := make(map[string]bool, len(result.Gen))
	for _, r := range result.Gen {
		desired[r.Kind()+"\x00"+r.Name()] = true
	}
	if file == nil {
		return result
	}
	for _, existing := range file.Rules {
		if _, owned := kinds[existing.Kind()]; !owned || desired[existing.Kind()+"\x00"+existing.Name()] {
			continue
		}
		if owns != nil && !owns(existing) {
			continue
		}
		result.Empty = append(result.Empty, rule.NewRule(existing.Kind(), existing.Name()))
	}
	return result
}

func GeneratedBinary(r *rule.Rule, attr string) bool {
	target, ok := strings.CutSuffix(r.Name(), BinaryName(""))
	return ok && target != "" && slices.Contains(r.AttrStrings(attr), ":"+target)
}

func CoveredBinary(file *rule.File, kind, want string, covers func(*rule.Rule) bool) bool {
	if file == nil {
		return false
	}
	return slices.ContainsFunc(file.Rules, func(r *rule.Rule) bool {
		return r.Kind() == kind && r.Name() != want && covers(r)
	})
}

func ExportedFiles(file *rule.File) map[string]bool {
	files := make(map[string]bool)
	if file == nil {
		return files
	}
	for _, r := range file.Rules {
		if r.Kind() != "exports_files" || len(r.Args()) == 0 {
			continue
		}
		list, ok := r.Args()[0].(*build.ListExpr)
		if !ok {
			continue
		}
		for _, item := range list.List {
			if s, ok := item.(*build.StringExpr); ok {
				files[s.Value] = true
			}
		}
	}
	return files
}

func FormatMatches(matches []resolve.FindResult) string {
	labels := make([]string, 0, len(matches))
	for _, match := range matches {
		labels = append(labels, match.Label.String())
	}
	sort.Strings(labels)
	return fmt.Sprintf("[%s]", strings.Join(labels, ", "))
}

type SingleFileSpec struct {
	LanguageName string
	LibraryKind  string
	IsSource     func(name string) bool
	OwnsByName   bool
	TargetName   func(name string) (string, error)
	ParseImports func(content []byte, dir string) []ImportRef
	IsStdLib     func(name string) bool
	Candidates   func(name string) []string
	Wrap         func(imports []ImportRef) any
}

func mappedByOverride(c *config.Config, lang string, candidates func(string) []string, name string) bool {
	if c == nil || c.Exts["_resolve"] == nil {
		return false
	}
	names := []string{name}
	if candidates != nil {
		names = append(names, candidates(name)...)
	}
	for _, n := range names {
		if _, found := resolve.FindRuleWithOverride(c, resolve.ImportSpec{Lang: lang, Imp: n}, lang); found {
			return true
		}
	}
	return false
}

func skipStdLib(args language.GenerateArgs, lang string, isStdLib func(string) bool, candidates func(string) []string, name string) bool {
	return isStdLib != nil && isStdLib(name) && !mappedByOverride(args.Config, lang, candidates, name)
}

func GenerateSingleFile(args language.GenerateArgs, kinds map[string]rule.KindInfo, spec SingleFileSpec, rep Reporter) language.GenerateResult {
	var sources []string
	for _, name := range args.RegularFiles {
		if spec.IsSource(name) {
			sources = append(sources, name)
		}
	}
	sort.Strings(sources)
	if len(sources) == 0 {
		return MergeStaleOwned(args.File, language.GenerateResult{}, kinds, singleFileOwns(args, spec, nil))
	}
	type plan struct {
		name    string
		src     string
		imports []ImportRef
	}
	var plans []plan
	for _, src := range sources {
		content, err := ReadSource(filepath.Join(args.Dir, src))
		if err != nil {
			rep.Fail(InPackage(args.Rel), "%s: %s: read %s: %v", spec.LanguageName, args.Rel, src, err)
			continue
		}
		name, err := spec.TargetName(src)
		if err != nil {
			rep.Fail(InPackage(args.Rel), "%s: %s: %v", spec.LanguageName, args.Rel, err)
			continue
		}
		p := plan{name: name, src: src}
		seen := make(map[refKey]bool)
		for _, ref := range spec.ParseImports(content, path.Dir(path.Join(args.Rel, src))) {
			key := refKey{ref.Root, ref.ModuleExt}
			if seen[key] || (!ref.Relative && skipStdLib(args, spec.LanguageName, spec.IsStdLib, spec.Candidates, ref.Root)) {
				continue
			}
			seen[key] = true
			p.imports = append(p.imports, ref)
		}
		SortRefs(p.imports)
		plans = append(plans, p)
	}
	if rep.Failed() {
		return language.GenerateResult{}
	}
	claimants := make([]Claimant, 0, len(plans))
	for _, p := range plans {
		claimants = append(claimants, Claimant{Name: p.name, Source: p.src, Kind: spec.LibraryKind})
	}
	if err := CheckClaims(args.File, args.OtherGen, claimants); err != nil {
		rep.Fail(InPackage(args.Rel), "%s: %s: %v", spec.LanguageName, args.Rel, err)
		return language.GenerateResult{}
	}
	var result language.GenerateResult
	for _, p := range plans {
		r := rule.NewRule(spec.LibraryKind, p.name)
		r.SetAttr("srcs", []string{p.src})
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, spec.Wrap(append([]ImportRef(nil), p.imports...)))
	}
	if FixtureTestonly(args.Config, args.Rel) {
		for _, r := range result.Gen {
			r.SetAttr("testonly", true)
		}
	}
	generated := make(map[string]bool, len(plans))
	for _, p := range plans {
		generated[p.name] = true
	}
	return MergeStaleOwned(args.File, result, kinds, singleFileOwns(args, spec, generated))
}

func singleFileOwns(args language.GenerateArgs, spec SingleFileSpec, generated map[string]bool) func(*rule.Rule) bool {
	if !spec.OwnsByName {
		return nil
	}
	return func(r *rule.Rule) bool {
		if generated[r.Name()] {
			return true
		}
		for _, src := range r.AttrStrings("srcs") {
			if _, err := os.Stat(filepath.Join(args.Dir, filepath.FromSlash(src))); err == nil {
				return false
			}
		}
		return true
	}
}

var utf8BOM = []byte{0xEF, 0xBB, 0xBF}

func StripBOM(content []byte) []byte {
	return bytes.TrimPrefix(content, utf8BOM)
}

func ReadSource(path string) ([]byte, error) {
	content, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	return StripBOM(content), nil
}

type KindAttr struct {
	Name  string
	Value any
}

type KindVariant struct {
	Kind         string
	DefsPackage  string
	Selects      func(root string) bool
	IsPlatform   func(root string) bool
	Attrs        func(pkg string) []KindAttr
	NeedsPackage string
}

type SingleDirSpec struct {
	Variant         *KindVariant
	OwnsByName      bool
	LanguageName    string
	LibraryKind     string
	IsSource        func(name string) bool
	IsHeader        func(name string) bool
	IsMainFile      func(name string) bool
	DirName         func(dir string) (string, error)
	RootHint        func(args language.GenerateArgs) string
	CheckPackage    bool
	ParsePackage    func(content []byte) (string, error)
	DefinesMain     func(content []byte) bool
	MainRule        string
	MainPhrase      string
	ParseImports    func(content []byte) []string
	IsStdLib        func(name string) bool
	Candidates      func(name string) []string
	OrderSrcs       func(name string, sources []string, contents map[string][]byte) ([]string, error)
	MakeRule        func(name string, sources []string, headers []string) *rule.Rule
	Extend          func(r *rule.Rule, files []string, contents map[string][]byte) error
	Wrap            func(imports []string) any
	Tests           *TestSpec
	KeepLibraryName bool
}

func GenerateSingleDir(args language.GenerateArgs, kinds map[string]rule.KindInfo, spec SingleDirSpec, rep Reporter) language.GenerateResult {
	var sources []string
	var headers []string
	var tests []string
	for _, name := range args.RegularFiles {
		switch {
		case spec.Tests != nil && spec.Tests.IsSource(name):
			tests = append(tests, name)
		case spec.IsSource(name):
			sources = append(sources, name)
		case spec.IsHeader != nil && spec.IsHeader(name):
			headers = append(headers, name)
		}
	}
	claimed := handwrittenSources(args.File, kinds)
	for src := range handwrittenTestSources(args.File, spec.Tests) {
		claimed[src] = true
	}
	sources = slices.DeleteFunc(sources, func(name string) bool { return claimed[name] })
	headers = slices.DeleteFunc(headers, func(name string) bool { return claimed[name] })
	sort.Strings(sources)
	sort.Strings(headers)
	sort.Strings(tests)
	testOwns := ownsGeneratedTest(spec.Tests)
	if len(sources) == 0 && len(headers) == 0 && len(tests) == 0 {
		return MergeStaleOwned(args.File, language.GenerateResult{}, kinds, withoutNestedSources(args.Dir, testOwns))
	}
	hasLib := len(sources) > 0 || len(headers) > 0
	var lib dirLibrary
	if hasLib {
		var ok bool
		lib, ok = generateDirLibrary(args, spec, sources, headers, rep)
		if !ok {
			return language.GenerateResult{}
		}
		hasLib = !lib.empty
	}
	type testPlan struct {
		name    string
		src     string
		rule    *rule.Rule
		imports []string
	}
	var plans []testPlan
	if spec.Tests != nil {
		owned := testOwners(args.File, args.OtherGen, spec.Tests, tests)
		for _, src := range tests {
			if owned[src] {
				continue
			}
			testName, err := spec.Tests.Name(src)
			if err != nil {
				rep.Fail(InPackage(args.Rel), "%s: %s: %v", spec.LanguageName, args.Rel, err)
				continue
			}
			content, err := ReadSource(filepath.Join(args.Dir, src))
			if err != nil {
				rep.Fail(InPackage(args.Rel), "%s: %s: read %s: %v", spec.LanguageName, args.Rel, src, err)
				continue
			}
			r, err := spec.Tests.makeRule(testName, src, content)
			if err != nil {
				rep.Fail(InPackage(args.Rel), "%s: %s: %s: %v", spec.LanguageName, args.Rel, src, err)
				continue
			}
			p := testPlan{name: testName, src: src, rule: r}
			seen := make(map[string]bool)
			for _, root := range spec.ParseImports(content) {
				if seen[root] || skipStdLib(args, spec.LanguageName, spec.IsStdLib, spec.Candidates, root) ||
					(spec.Tests.IsImplicit != nil && spec.Tests.IsImplicit(root)) {
					continue
				}
				seen[root] = true
				p.imports = append(p.imports, root)
			}
			sort.Strings(p.imports)
			plans = append(plans, p)
		}
		if rep.Failed() {
			return language.GenerateResult{}
		}
	}
	var claimants []Claimant
	if hasLib {
		libKinds := []string{spec.LibraryKind}
		if spec.Variant != nil {
			libKinds = append(libKinds, spec.Variant.Kind)
		}
		claimants = append(claimants, Claimant{Name: lib.name, Source: args.Rel, Kind: lib.kind, Kinds: libKinds})
	}
	for _, p := range plans {
		testKinds := append([]string{spec.Tests.Kind}, spec.Tests.AltKinds...)
		claimants = append(claimants, Claimant{Name: p.name, Source: path.Join(args.Rel, p.src), Kind: p.rule.Kind(), Kinds: testKinds})
	}
	if err := CheckClaims(args.File, args.OtherGen, claimants); err != nil {
		rep.Fail(InPackage(args.Rel), "%s: %s: %v", spec.LanguageName, args.Rel, err)
		return language.GenerateResult{}
	}
	result := language.GenerateResult{}
	if hasLib {
		if FixtureTestonly(args.Config, args.Rel) {
			lib.rule.SetAttr("testonly", true)
		}
		result.Gen = append(result.Gen, lib.rule)
		result.Imports = append(result.Imports, spec.Wrap(lib.imports))
	}
	for _, p := range plans {
		if hasLib {
			p.rule.SetAttr("deps", []string{":" + lib.name})
		}
		result.Gen = append(result.Gen, p.rule)
		result.Imports = append(result.Imports, spec.Wrap(p.imports))
	}
	var owns func(*rule.Rule) bool
	if spec.OwnsByName {
		owns = func(r *rule.Rule) bool {
			if hasLib && r.Name() == lib.name {
				return true
			}
			for _, attr := range []string{"srcs", "hdrs"} {
				for _, src := range r.AttrStrings(attr) {
					if _, err := os.Stat(filepath.Join(args.Dir, filepath.FromSlash(src))); err == nil {
						return false
					}
				}
			}
			return true
		}
	}
	if testOwns != nil {
		byName := owns
		owns = func(r *rule.Rule) bool {
			return testOwns(r) && (byName == nil || byName(r))
		}
	}
	owns = withoutNestedSources(args.Dir, owns)
	if owns != nil {
		claimed := make(map[string]string, len(claimants))
		for _, c := range claimants {
			claimed[c.Name] = c.Kind
		}
		inner := owns
		owns = func(r *rule.Rule) bool {
			if kind, ok := claimed[r.Name()]; ok && kind != r.Kind() {
				return true
			}
			return inner(r)
		}
	}
	return MergeStaleOwned(args.File, result, kinds, owns)
}

func withoutNestedSources(dir string, owns func(*rule.Rule) bool) func(*rule.Rule) bool {
	return func(r *rule.Rule) bool {
		for _, attr := range []string{"srcs", "hdrs"} {
			for _, src := range r.AttrStrings(attr) {
				if !strings.Contains(src, "/") || strings.HasPrefix(src, "//") || strings.HasPrefix(src, "@") {
					continue
				}
				if _, err := os.Stat(filepath.Join(dir, filepath.FromSlash(src))); err == nil {
					return false
				}
			}
		}
		return owns == nil || owns(r)
	}
}

type dirLibrary struct {
	name    string
	kind    string
	rule    *rule.Rule
	imports []string
	empty   bool
}

func handwrittenSources(file *rule.File, kinds map[string]rule.KindInfo) map[string]bool {
	named := make(map[string]bool)
	if file == nil || file.File == nil {
		return named
	}
	build.Walk(file.File, func(x build.Expr, _ []build.Expr) {
		call, ok := x.(*build.CallExpr)
		if !ok {
			return
		}
		if fn, ok := call.X.(*build.Ident); ok {
			if _, owned := kinds[fn.Name]; owned || fn.Name == "filegroup" {
				return
			}
		}
		for _, arg := range call.List {
			assign, ok := arg.(*build.AssignExpr)
			if !ok {
				continue
			}
			key, ok := assign.LHS.(*build.Ident)
			if !ok || !slices.Contains([]string{"srcs", "hdrs", "main", "struct"}, key.Name) {
				continue
			}
			build.Walk(assign.RHS, func(v build.Expr, _ []build.Expr) {
				if s, ok := v.(*build.StringExpr); ok {
					named[s.Value] = true
				}
			})
		}
	})
	return named
}

func exportedFiles(file *rule.File) map[string]bool {
	exported := make(map[string]bool)
	if file == nil {
		return exported
	}
	for _, r := range file.Rules {
		if r.Kind() != "exports_files" {
			continue
		}
		for _, arg := range r.Args() {
			build.Walk(arg, func(v build.Expr, _ []build.Expr) {
				if s, ok := v.(*build.StringExpr); ok {
					exported[s.Value] = true
				}
			})
		}
	}
	return exported
}

func mainOwners(file *rule.File, kind string) map[string]bool {
	owned := exportedFiles(file)
	if file == nil || kind == "" {
		return owned
	}
	for _, r := range file.Rules {
		if r.Kind() != kind {
			continue
		}
		for _, src := range r.AttrStrings("srcs") {
			owned[strings.TrimPrefix(src, ":")] = true
		}
	}
	return owned
}

func ExistingLibraryName(file *rule.File, kind string, sources []string) (string, bool) {
	if file == nil {
		return "", false
	}
	have := make(map[string]bool, len(sources))
	for _, src := range sources {
		have[src] = true
	}
	for _, r := range file.Rules {
		if r.Kind() != kind {
			continue
		}
		srcs := r.AttrStrings("srcs")
		if len(srcs) == 0 {
			continue
		}
		all := true
		for _, src := range srcs {
			if !have[path.Clean(src)] {
				all = false
				break
			}
		}
		if all {
			return r.Name(), true
		}
	}
	return "", false
}

func generateDirLibrary(args language.GenerateArgs, spec SingleDirSpec, sources, headers []string, rep Reporter) (dirLibrary, bool) {
	var hints []string
	if args.Rel == "" && spec.RootHint != nil {
		hints = append(hints, spec.RootHint(args))
	}
	name, err := PackageTargetName(args, hints...)
	if args.Rel != "" {
		name, err = spec.DirName(args.Rel)
	}
	if err != nil {
		rep.Fail(InPackage(args.Rel), "%s: %s: %v", spec.LanguageName, args.Rel, err)
		return dirLibrary{}, false
	}
	isMain := spec.IsMainFile
	if isMain == nil {
		isMain = func(string) bool { return true }
	}
	owned := mainOwners(args.File, spec.MainRule)
	skipped := make(map[string]bool)
	packages := make(map[string]bool)
	seen := make(map[string]bool)
	contents := make(map[string][]byte, len(sources))
	var raw []string
	for _, file := range append(append([]string{}, sources...), headers...) {
		content, err := ReadSource(filepath.Join(args.Dir, file))
		if err != nil {
			rep.Fail(InPackage(args.Rel), "%s: %s: read %s: %v", spec.LanguageName, args.Rel, file, err)
			continue
		}
		contents[file] = content
		if isMain(file) && spec.DefinesMain(content) {
			if owned[file] {
				skipped[file] = true
				continue
			}
			rep.Fail(InPackage(args.Rel), "%s: %s: %s defines main; thin %s entries stay handwritten, so split %s into their own directory before adopting generation", spec.LanguageName, args.Rel, file, spec.MainRule, spec.MainPhrase)
			continue
		}
		if spec.CheckPackage {
			pkg, err := spec.ParsePackage(content)
			if err != nil {
				rep.Fail(InPackage(args.Rel), "%s: %s: parse package %s: %v", spec.LanguageName, args.Rel, file, err)
				continue
			}
			packages[pkg] = true
		}
		for _, root := range spec.ParseImports(content) {
			if seen[root] {
				continue
			}
			seen[root] = true
			raw = append(raw, root)
		}
	}
	if rep.Failed() {
		return dirLibrary{}, false
	}
	if len(packages) > 1 {
		names := make([]string, 0, len(packages))
		for pkg := range packages {
			names = append(names, pkg)
		}
		sort.Strings(names)
		rep.Fail(InPackage(args.Rel), "%s: %s: mixed packages %s in one directory; split the directory before adopting generation", spec.LanguageName, args.Rel, strings.Join(names, ", "))
		return dirLibrary{}, false
	}
	libraryKind := spec.LibraryKind
	variant := spec.Variant
	chosen := false
	if variant != nil {
		for _, root := range raw {
			if variant.Selects(root) && !mappedByOverride(args.Config, spec.LanguageName, spec.Candidates, root) {
				chosen = true
				break
			}
		}
	}
	var pkg string
	if chosen {
		libraryKind = variant.Kind
		for p := range packages {
			pkg = p
		}
		if pkg == "" {
			rep.Fail(InPackage(args.Rel), "%s: %s: %s", spec.LanguageName, args.Rel, variant.NeedsPackage)
			return dirLibrary{}, false
		}
	}
	var imports []string
	for _, root := range raw {
		if chosen && variant.IsPlatform(root) && !mappedByOverride(args.Config, spec.LanguageName, spec.Candidates, root) {
			continue
		}
		if skipStdLib(args, spec.LanguageName, spec.IsStdLib, spec.Candidates, root) {
			continue
		}
		imports = append(imports, root)
	}
	sort.Strings(imports)
	if len(skipped) > 0 {
		kept := make([]string, 0, len(sources))
		for _, src := range sources {
			if !skipped[src] {
				kept = append(kept, src)
			}
		}
		sources = kept
	}
	if spec.KeepLibraryName {
		if kept, ok := ExistingLibraryName(args.File, spec.LibraryKind, sources); ok {
			name = kept
		}
	}
	ordered := sources
	if spec.OrderSrcs != nil {
		ordered, err = spec.OrderSrcs(name, sources, contents)
		if err != nil {
			rep.Fail(InPackage(args.Rel), "%s: %s: %v", spec.LanguageName, args.Rel, err)
			return dirLibrary{}, false
		}
	}
	makeRule := spec.MakeRule
	if makeRule == nil {
		makeRule = func(ruleName string, srcs []string, _ []string) *rule.Rule {
			r := rule.NewRule(libraryKind, ruleName)
			r.SetAttr("srcs", srcs)
			return r
		}
	}
	lib := makeRule(name, ordered, headers)
	lib.SetKind(libraryKind)
	if spec.Extend != nil {
		if err := spec.Extend(lib, args.RegularFiles, contents); err != nil {
			rep.Fail(InPackage(args.Rel), "%s: %s: %v", spec.LanguageName, args.Rel, err)
			return dirLibrary{}, false
		}
	}
	if chosen {
		for _, a := range variant.Attrs(pkg) {
			lib.SetAttr(a.Name, a.Value)
		}
	}
	return dirLibrary{name: name, kind: lib.Kind(), rule: lib, imports: imports, empty: len(sources) == 0 && len(headers) == 0}, true
}

type ResolveSpec struct {
	LanguageName        string
	WantKind            string
	Attr                string
	Unwrap              func(raw any) ([]string, bool)
	UnwrapRefs          func(raw any) ([]ImportRef, bool)
	Fallback            []string
	Find                func(c *config.Config, ix *resolve.RuleIndex, ref ImportRef) JSLookup
	IsStdLib            func(name string) bool
	Candidates          func(name string) []string
	Lookups             func(pkg, name string) []string
	MarkUsed            func(name string) bool
	SkipDep             func(dep label.Label) bool
	External            func(name string) ([]label.Label, string)
	MissingExternalHint func(pkg, name string) string
	FailConflict        func(from label.Label, name string)
	FailUnresolved      func(from label.Label, name string, hint string)
	FailAmbiguous       func(from label.Label, name string, matches string)
}

func ResolveSingle(c *config.Config, ix *resolve.RuleIndex, r *rule.Rule, raw any, from label.Label, spec ResolveSpec) {
	refs, ok := unwrapRefs(raw, spec)
	if !ok {
		return
	}
	if r.Kind() != spec.WantKind {
		return
	}
	deps := make(map[string]bool)
	add := func(dep label.Label) {
		if dep == from || (spec.SkipDep != nil && spec.SkipDep(dep)) {
			return
		}
		if dep.Repo == "" && !dep.Relative {
			dep.Repo = from.Repo
		}
		deps[dep.Rel(from.Repo, from.Pkg).String()] = true
	}
	for _, ref := range refs {
		name := ref.Root
		var matches []resolve.FindResult
		if spec.Find != nil {
			lk := spec.Find(c, ix, ref)
			if lk.HasOverride {
				if spec.MarkUsed(name) {
					spec.FailConflict(from, name)
					continue
				}
				add(lk.Override)
				continue
			}
			if !ref.Relative && spec.IsStdLib != nil && spec.IsStdLib(name) {
				continue
			}
			matches = lk.Matches
		} else {
			var done bool
			matches, done = lookupOwn(c, ix, spec, ref, from, add)
			if done {
				continue
			}
		}
		if len(matches) == 0 && ref.Relative && len(spec.Fallback) > 0 {
			fb, mapped := findFallback(c, ix, spec.Fallback, name, from)
			if mapped {
				add(fb.Override)
				continue
			}
			matches = fb.Matches
			if len(matches) == 0 && fb.SelfMatch {
				continue
			}
		}
		if len(matches) == 0 && ref.Relative && ref.Ext != "" {
			spec.MarkUsed(name)
			continue
		}
		switch len(matches) {
		case 1:
			add(matches[0].Label)
		case 0:
			if spec.MarkUsed(name) {
				continue
			}
			hint := ""
			if spec.External != nil && !ref.Relative {
				var ext []label.Label
				ext, hint = spec.External(name)
				switch {
				case len(ext) == 1:
					if ext[0] != from {
						deps[ext[0].Rel(from.Repo, from.Pkg).String()] = true
					}
					continue
				case len(ext) > 1:
					spec.FailAmbiguous(from, name, formatLabels(ext))
					continue
				}
			}
			if hint == "" && !ref.Relative && spec.MissingExternalHint != nil {
				hint = spec.MissingExternalHint(from.Pkg, name)
			}
			spec.FailUnresolved(from, name, hint)
		default:
			if slices.ContainsFunc(matches, func(m resolve.FindResult) bool { return m.Label == from }) {
				continue
			}
			spec.FailAmbiguous(from, name, FormatMatches(matches))
		}
	}
	if len(deps) == 0 {
		return
	}
	labels := make([]string, 0, len(deps))
	for dep := range deps {
		labels = append(labels, dep)
	}
	sort.Strings(labels)
	have := r.AttrStrings(spec.Attr)
	if union := UnionStrings(have, labels); !slices.Equal(union, have) {
		r.SetAttr(spec.Attr, union)
	}
}

func lookupOwn(c *config.Config, ix *resolve.RuleIndex, spec ResolveSpec, ref ImportRef, from label.Label, add func(label.Label)) (matches []resolve.FindResult, done bool) {
	name := ref.Root
	imp := resolve.ImportSpec{Lang: spec.LanguageName, Imp: name}
	override, found := resolve.FindRuleWithOverride(c, imp, spec.LanguageName)
	if !found && !ref.Relative && spec.IsStdLib != nil && spec.IsStdLib(name) {
		return nil, true
	}
	if found {
		if spec.MarkUsed(name) {
			spec.FailConflict(from, name)
			return nil, true
		}
		add(override)
		return nil, true
	}
	ownIndex := !ref.Relative || !slices.Contains(JSModuleExts, ref.ModuleExt)
	keys := []string{name}
	if spec.Lookups != nil {
		keys = spec.Lookups(from.Pkg, name)
	}
	mappedKey := false
	for _, key := range keys {
		if !ownIndex {
			break
		}
		kimp := resolve.ImportSpec{Lang: spec.LanguageName, Imp: key}
		if key != name {
			if override, found := resolve.FindRuleWithOverride(c, kimp, spec.LanguageName); found {
				add(override)
				mappedKey = true
				break
			}
		}
		if matches = ix.FindRulesByImportWithConfig(c, kimp, spec.LanguageName); len(matches) > 0 {
			break
		}
	}
	if mappedKey {
		return nil, true
	}
	if len(matches) == 0 && ownIndex {
		var cands []string
		if spec.Candidates != nil {
			cands = spec.Candidates(name)
		}
		mapped := false
		for _, cand := range cands {
			cimp := resolve.ImportSpec{Lang: spec.LanguageName, Imp: cand}
			if override, found := resolve.FindRuleWithOverride(c, cimp, spec.LanguageName); found {
				add(override)
				mapped = true
				break
			}
			if matches = ix.FindRulesByImportWithConfig(c, cimp, spec.LanguageName); len(matches) > 0 {
				break
			}
		}
		if mapped {
			return nil, true
		}
	}
	return matches, false
}

func unwrapRefs(raw any, spec ResolveSpec) ([]ImportRef, bool) {
	if spec.UnwrapRefs != nil {
		if refs, ok := spec.UnwrapRefs(raw); ok {
			return refs, true
		}
	}
	if spec.Unwrap == nil {
		return nil, false
	}
	names, ok := spec.Unwrap(raw)
	if !ok {
		return nil, false
	}
	refs := make([]ImportRef, 0, len(names))
	for _, name := range names {
		refs = append(refs, ImportRef{Spec: name, Root: name})
	}
	return refs, true
}

type fallbackLookup struct {
	Override  label.Label
	Matches   []resolve.FindResult
	SelfMatch bool
}

func findFallback(c *config.Config, ix *resolve.RuleIndex, langs []string, name string, from label.Label) (fallbackLookup, bool) {
	var out fallbackLookup
	seen := make(map[label.Label]bool)
	for _, lang := range langs {
		found := FindJSImport(c, ix, lang, name, true)
		if found.HasOverride {
			return fallbackLookup{Override: found.Override}, true
		}
		for _, m := range found.Matches {
			if m.Label == from {
				out.SelfMatch = true
				continue
			}
			if !seen[m.Label] {
				seen[m.Label] = true
				out.Matches = append(out.Matches, m)
			}
		}
	}
	if len(out.Matches) > 0 {
		out.SelfMatch = false
	}
	return out, false
}

func formatLabels(labels []label.Label) string {
	out := make([]string, 0, len(labels))
	for _, l := range labels {
		out = append(out, l.String())
	}
	sort.Strings(out)
	return fmt.Sprintf("[%s]", strings.Join(out, ", "))
}
