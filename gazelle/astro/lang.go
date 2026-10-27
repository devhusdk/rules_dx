package astro

import (
	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazelbuild/bazel-gazelle/language"
)

func NewLanguage() language.Language {
	return common.NewWebComponentLang("astro", []string{".astro"}, ParseImports)
}
