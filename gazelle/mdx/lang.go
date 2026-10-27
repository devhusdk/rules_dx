package mdx

import (
	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazelbuild/bazel-gazelle/language"
)

func NewLanguage() language.Language {
	return common.NewWebComponentLang("mdx", []string{".mdx"}, ParseImports)
}
