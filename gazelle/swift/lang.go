package swift

import (
	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

const languageName = "swift"

var swiftKinds = map[string]rule.KindInfo{
	LibraryKind:  common.LibraryKindInfo(),
	TestKind:     common.TestKindInfo(),
	IOSAppKind:   appKindInfo,
	MacOSAppKind: appKindInfo,
}

type swiftLang struct {
	*common.SingleLang
}

func NewLanguage() language.Language {
	return &swiftLang{SingleLang: common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		Kinds:          swiftKinds,
		ExtraKinds:     []string{TestKind, IOSAppKind, MacOSAppKind},
		DefsPackage:    "swift:defs.bzl",
		KindDefs:       map[string]string{IOSAppKind: appleDefs, MacOSAppKind: appleDefs},
		ImportAttr:     "srcs",
		IsImportSource: isSupported,
		IsTestSource:   IsTestSource,
		IsStdLib:       IsStdLib,
		External:       "swiftpm",
		UnresolvedHint: "add a swift_library with that name or module_name, or an exact # gazelle:resolve swift mapping",
		Generate:       generateRules,
	})}
}

func (l *swiftLang) Imports(_ *config.Config, r *rule.Rule, _ *rule.File) []resolve.ImportSpec {
	if r.Kind() != LibraryKind {
		return nil
	}
	name := r.AttrString("module_name")
	if name == "" {
		name = r.Name()
	}
	return []resolve.ImportSpec{{Lang: languageName, Imp: name}}
}

func isSupported(name string) bool { return common.HasExt(name, SupportedExts) }

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	entries, appKind := appEntries(args.Dir, args.RegularFiles)
	res := common.GenerateSingleDir(args, kinds, common.SingleDirSpec{
		LanguageName: languageName,
		LibraryKind:  LibraryKind,
		IsSource: func(file string) bool {
			return isSupported(file) && !IsTestSource(file) && !IsEntryFile(file)
		},
		DirName:      DirTargetName,
		IsMainFile:   func(file string) bool { return !entries[file] },
		DefinesMain:  DefinesMain,
		MainRule:     "swift_binary",
		MainPhrase:   "@main sources",
		ParseImports: ParseImports,
		IsStdLib:     IsStdLib,
		Wrap:         common.WrapImportSet,
		Tests: &common.TestSpec{
			Kind:     TestKind,
			IsSource: func(file string) bool { return isSupported(file) && IsTestSource(file) },
			Name:     func(src string) (string, error) { return common.SnakeName(src, SupportedExts) },
		},
	}, rep)
	var apps []*rule.Rule
	for _, r := range res.Gen {
		if r.Kind() != LibraryKind {
			continue
		}
		if r.AttrString("module_name") == "" {
			r.SetAttr("module_name", ModuleName(r.Name()))
		}
		if appKind != "" && ownsEntry(r, entries) {
			apps = append(apps, appRule(appKind, r.Name()))
		}
	}
	for _, app := range apps {
		res.Gen = append(res.Gen, app)
		res.Imports = append(res.Imports, nil)
	}
	return res
}

func init() { common.RegisterLanguage(NewLanguage) }

func ownsEntry(r *rule.Rule, entries map[string]bool) bool {
	for _, src := range r.AttrStrings("srcs") {
		if entries[src] {
			return true
		}
	}
	return false
}
