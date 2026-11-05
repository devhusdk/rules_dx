package protobuf

import (
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

var kinds = map[string]rule.KindInfo{
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

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	return common.GenerateSingleDir(args, kinds, common.SingleDirSpec{
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
}

func init() { common.RegisterLanguage(NewLanguage) }
