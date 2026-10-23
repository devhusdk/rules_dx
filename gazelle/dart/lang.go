package dart

import (
	upstream "github.com/aran/rules_dart/gazelle/dart"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/rule"
)

const defsPackage = "dart/rules:defs.bzl"

var wrappedKinds = []string{"dart_binary", "dart_codegen", "dart_library", "dart_test"}

type dartLang struct {
	language.Language
}

func NewLanguage() language.Language {
	return &dartLang{Language: upstream.NewLanguage()}
}

func (l *dartLang) Loads() []rule.LoadInfo { return l.loads("rules_dx") }

func (l *dartLang) ApparentLoads(moduleToApparentName func(string) string) []rule.LoadInfo {
	repoName := moduleToApparentName("rules_dx")
	if repoName == "" {
		repoName = "rules_dx"
	}
	return l.loads(repoName)
}

func (l *dartLang) loads(rulesRepo string) []rule.LoadInfo {
	wrapped := map[string]bool{}
	for _, kind := range wrappedKinds {
		wrapped[kind] = true
	}
	loads := []rule.LoadInfo{{Name: "@" + rulesRepo + "//" + defsPackage, Symbols: wrappedKinds}}
	for _, info := range l.Language.Loads() {
		var symbols []string
		for _, symbol := range info.Symbols {
			if !wrapped[symbol] {
				symbols = append(symbols, symbol)
			}
		}
		if len(symbols) == 0 {
			continue
		}
		info.Symbols = symbols
		loads = append(loads, info)
	}
	return loads
}
