package protobuf

import (
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

const languageName = "protobuf"

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
})

func withBindings(kinds map[string]rule.KindInfo) map[string]rule.KindInfo {
	for kind, info := range bindingKinds() {
		kinds[kind] = info
	}
	return kinds
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
		ExtraKinds:     bindingExtraKinds(),
		KindDefs:       bindingKindDefs(),
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
		l.resolveGoBinding(c, ix, r, raw, from)
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

// Imports indexes proto_library by its files and bindings by the code they
// generate.
func (l *protoLang) Imports(c *config.Config, r *rule.Rule, f *rule.File) []resolve.ImportSpec {
	for _, b := range bindings {
		if r.Kind() == b.kind {
			return bindingImports(c, r, f)
		}
	}
	return l.SingleLang.Imports(c, r, f)
}

// resolveGoBinding sets the deps of a go_proto_library to the Go bindings of
// the proto_library targets its proto imports.
func (l *protoLang) resolveGoBinding(c *config.Config, ix *resolve.RuleIndex, r *rule.Rule, raw interface{}, from label.Label) {
	names, ok := common.UnwrapImportSet(raw)
	if !ok {
		return
	}
	spec := l.ResolveSpecFor(c, r, from)
	deps := map[string]bool{}
	for _, name := range names {
		if strings.HasPrefix(name, "google/protobuf/") {
			continue
		}
		owners := ix.FindRulesByImportWithConfig(c, resolve.ImportSpec{Lang: languageName, Imp: name}, languageName)
		if len(owners) != 1 {
			continue
		}
		own := bindingKey(keyGoProto, owners[0].Label.String())
		found := ix.FindRulesByImportWithConfig(c, resolve.ImportSpec{Lang: languageName, Imp: own}, languageName)
		switch len(found) {
		case 0:
			spec.FailUnresolved(from, name, "the proto_library has no Go binding; add option go_package to it")
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
	return addBindings(args, result)
}

func init() { common.RegisterLanguage(NewLanguage) }
