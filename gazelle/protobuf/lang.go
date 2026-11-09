package protobuf

import (
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/repo"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

const (
	languageName = "protobuf"
	rustKind     = "rust_prost_library"
	rustSuffix   = "_rs"
	rustModule   = "rules_rust_prost"
	rustDefs     = "@rules_rust_prost//:defs.bzl"
	tsKind       = "typescript_proto_library"
	tsSuffix     = "_ts"
	tsModule     = "aspect_rules_ts"
	tsRuntime    = "//:node_modules/@bufbuild/protobuf"
	tsDefs       = "@rules_dx//typescript/rules:defs.bzl"
	csKind       = "csharp_proto_library"
	csSuffix     = "_cs"
	csModule     = "rules_dx_dotnet"
	csRuntime    = "@paket.main//google.protobuf"
	csDefs       = "@rules_dx_dotnet//csharp/rules:defs.bzl"
)

var kinds = withBindings(map[string]rule.KindInfo{
	LibraryKind: {
		MatchAttrs:    []string{"srcs"},
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"deps":                true,
			"srcs":                true,
			"strip_import_prefix": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	},
	rustKind: protoRefKind(),
	tsKind:   tsKindInfo(),
	csKind:   csKindInfo(),
})

func protoRefKind() rule.KindInfo {
	return rule.KindInfo{
		MatchAttrs:     []string{"proto"},
		NonEmptyAttrs:  map[string]bool{"proto": true},
		MergeableAttrs: map[string]bool{"proto": true},
	}
}

func tsKindInfo() rule.KindInfo {
	info := protoRefKind()
	info.MergeableAttrs = map[string]bool{"proto": true, "deps": true}
	return info
}

func csKindInfo() rule.KindInfo {
	info := tsKindInfo()
	info.ResolveAttrs = map[string]bool{"deps": true}
	return info
}

func withBindings(kinds map[string]rule.KindInfo) map[string]rule.KindInfo {
	for kind, info := range bindingKinds() {
		kinds[kind] = info
	}
	return kinds
}

// extraKinds are the kinds generated next to proto_library.
func extraKinds() []string {
	return append(bindingExtraKinds(), rustKind, tsKind, csKind)
}

func extraKindDefs() map[string]string {
	defs := bindingKindDefs()
	defs[rustKind] = rustDefs
	defs[tsKind] = tsDefs
	defs[csKind] = csDefs
	return defs
}

type protoLang struct {
	*common.SingleLang
}

func NewLanguage() language.Language {
	return &protoLang{SingleLang: common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		Kinds:          kinds,
		DefsPackage:    "@protobuf//bazel:proto_library.bzl",
		ExtraKinds:     extraKinds(),
		KindDefs:       extraKindDefs(),
		ImportAttr:     "srcs",
		Identity:       func(name string) string { return name },
		PathIdentity:   true,
		IsImportSource: func(name string) bool { return common.HasExt(name, SupportedExts) },
		UnresolvedHint: "add a proto_library that owns that file or an exact # gazelle:resolve protobuf mapping",
		Generate:       generateRules,
	})}
}

// Resolve sends google/protobuf/*.proto imports to the @protobuf targets
// when no local proto_library owns them.
func (l *protoLang) Resolve(c *config.Config, ix *resolve.RuleIndex, _ *repo.RemoteCache, r *rule.Rule, raw interface{}, from label.Label) {
	if r.Kind() == goKind {
		l.resolveBinding(c, ix, r, raw, from, keyGoProto, "the proto_library has no Go binding; add option go_package to it", nil)
		return
	}
	if r.Kind() == csKind {
		l.resolveBinding(c, ix, r, raw, from, keyCSProto, "the proto_library has no C# binding", []string{csRuntime})
		return
	}
	spec := l.ResolveSpecFor(c, r, from)
	spec.External = func(name string) ([]label.Label, string) {
		text, ok := WellKnownLabel(name)
		if !ok {
			return nil, ""
		}
		dep, err := label.Parse(text)
		if err != nil {
			return nil, ""
		}
		return []label.Label{dep}, ""
	}
	common.ResolveSingle(c, ix, r, raw, from, spec)
}

// Imports indexes proto_library by its files, bindings by the code they
// generate, and a rust_prost_library under its crate name.
func (l *protoLang) Imports(c *config.Config, r *rule.Rule, f *rule.File) []resolve.ImportSpec {
	if r.Kind() == rustKind {
		return []resolve.ImportSpec{{Lang: "rust", Imp: r.Name()}}
	}
	if r.Kind() == tsKind {
		return tsImports(r, f)
	}
	if r.Kind() == csKind {
		return csImports(c, r, f)
	}
	for _, b := range bindings {
		if r.Kind() == b.kind {
			return bindingImports(c, r, f)
		}
	}
	return l.SingleLang.Imports(c, r, f)
}

// resolveBinding sets the deps of a binding rule to base plus the bindings,
// found under key, of the proto_library targets its proto imports. hint is
// the error when an imported proto_library has no binding.
func (l *protoLang) resolveBinding(c *config.Config, ix *resolve.RuleIndex, r *rule.Rule, raw interface{}, from label.Label, key, hint string, base []string) {
	names, ok := common.UnwrapImportSet(raw)
	if !ok {
		return
	}
	spec := l.ResolveSpecFor(c, r, from)
	deps := map[string]bool{}
	for _, dep := range base {
		deps[dep] = true
	}
	for _, name := range names {
		if strings.HasPrefix(name, "google/protobuf/") {
			continue
		}
		owners := ix.FindRulesByImportWithConfig(c, resolve.ImportSpec{Lang: languageName, Imp: name}, languageName)
		if len(owners) != 1 {
			continue
		}
		own := bindingKey(key, owners[0].Label.String())
		found := ix.FindRulesByImportWithConfig(c, resolve.ImportSpec{Lang: languageName, Imp: own}, languageName)
		switch len(found) {
		case 0:
			spec.FailUnresolved(from, name, hint)
		case 1:
			if found[0].Label != from {
				deps[found[0].Label.Rel(from.Repo, from.Pkg).String()] = true
			}
		default:
			spec.FailAmbiguous(from, name, common.FormatMatches(found))
		}
	}
	if len(deps) == 0 {
		r.DelAttr("deps")
		return
	}
	labels := make([]string, 0, len(deps))
	for dep := range deps {
		labels = append(labels, dep)
	}
	sort.Strings(labels)
	r.SetAttr("deps", labels)
}

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	result := common.GenerateSingleDir(args, map[string]rule.KindInfo{LibraryKind: kinds[LibraryKind]}, common.SingleDirSpec{
		LanguageName: languageName,
		LibraryKind:  LibraryKind,
		IsSource:     func(file string) bool { return common.HasExt(file, SupportedExts) },
		IsMainFile:   func(string) bool { return false },
		DirName:      DirTargetName,
		ParseImports: ParseImports,
		MakeRule: func(name string, srcs []string, _ []string) *rule.Rule {
			if !strings.HasSuffix(name, suffix) {
				name += suffix
			}
			r := rule.NewRule(LibraryKind, name)
			r.SetAttr("srcs", srcs)
			r.SetAttr("strip_import_prefix", "/")
			return r
		},
		Wrap: common.WrapImportSet,
	}, rep)
	result = addRustLibraries(args, result)
	result = addTypeScriptLibraries(args, result)
	result = addCSharpLibraries(args, result)
	return addBindings(args, result)
}

// addRustLibraries adds a rust_prost_library next to each proto_library when
// the root MODULE.bazel depends on rules_rust_prost.
func addRustLibraries(args language.GenerateArgs, result language.GenerateResult) language.GenerateResult {
	return addProtoRefs(args, result, rustModule, rustKind, rustSuffix)
}

// addTypeScriptLibraries adds a typescript_proto_library next to each proto_library
// when the root MODULE.bazel depends on aspect_rules_ts.
func addTypeScriptLibraries(args language.GenerateArgs, result language.GenerateResult) language.GenerateResult {
	return addProtoRefs(args, result, tsModule, tsKind, tsSuffix)
}

// addCSharpLibraries adds a csharp_proto_library next to each proto_library
// when the root MODULE.bazel depends on rules_dx_dotnet.
func addCSharpLibraries(args language.GenerateArgs, result language.GenerateResult) language.GenerateResult {
	return addProtoRefs(args, result, csModule, csKind, csSuffix)
}

func addProtoRefs(args language.GenerateArgs, result language.GenerateResult, module, kind, suffix string) language.GenerateResult {
	if !common.ModuleDeps(args.Config.RepoRoot)[module] {
		return result
	}
	var refs []*rule.Rule
	var refImports []any
	for _, lib := range result.Gen {
		if lib.Kind() != LibraryKind {
			continue
		}
		r := rule.NewRule(kind, lib.Name()+suffix)
		r.SetAttr("proto", ":"+lib.Name())
		switch kind {
		case tsKind:
			r.SetAttr("deps", []string{tsRuntime})
		case csKind:
			r.SetAttr("deps", []string{csRuntime})
		}
		if common.ShouldSetVisibility(args) {
			r.SetAttr("visibility", []string{"//visibility:public"})
		}
		var imp any
		if kind == csKind {
			imp = common.WrapImportSet(libImports(result, lib))
		}
		refs = append(refs, r)
		refImports = append(refImports, imp)
	}
	result.Gen = append(result.Gen, refs...)
	result.Imports = append(result.Imports, refImports...)
	for _, r := range result.Empty {
		if r.Kind() == LibraryKind {
			result.Empty = append(result.Empty, rule.NewRule(kind, r.Name()+suffix))
		}
	}
	return result
}

// tsImports indexes a typescript_proto_library by the <stem>_pb modules protoc-gen-es
// writes for the sources of its proto_library.
func tsImports(r *rule.Rule, f *rule.File) []resolve.ImportSpec {
	if f == nil {
		return nil
	}
	var specs []resolve.ImportSpec
	for _, lib := range f.Rules {
		if ":"+lib.Name() != r.AttrString("proto") || lib.Kind() != LibraryKind {
			continue
		}
		for _, src := range lib.AttrStrings("srcs") {
			stem := strings.TrimSuffix(src, ".proto")
			specs = append(specs, resolve.ImportSpec{Lang: "typescript", Imp: path.Join(f.Pkg, stem+"_pb")})
		}
	}
	return specs
}

// csImports indexes a csharp_proto_library by the namespaces protoc writes for
// the sources of its proto_library.
func csImports(c *config.Config, r *rule.Rule, f *rule.File) []resolve.ImportSpec {
	if f == nil {
		return nil
	}
	var specs []resolve.ImportSpec
	seen := map[string]bool{}
	for _, lib := range f.Rules {
		if ":"+lib.Name() != r.AttrString("proto") || lib.Kind() != LibraryKind {
			continue
		}
		specs = append(specs, resolve.ImportSpec{Lang: languageName, Imp: bindingKey(keyCSProto, protoLabel(c, f.Pkg, lib.Name()))})
		for _, src := range lib.AttrStrings("srcs") {
			content, err := os.ReadFile(filepath.Join(c.RepoRoot, f.Pkg, src))
			if err != nil {
				continue
			}
			if ns := CSharpNamespace(content); ns != "" && !seen[ns] {
				seen[ns] = true
				specs = append(specs, resolve.ImportSpec{Lang: "csharp", Imp: ns})
			}
		}
	}
	return specs
}

func init() { common.RegisterLanguage(NewLanguage) }
