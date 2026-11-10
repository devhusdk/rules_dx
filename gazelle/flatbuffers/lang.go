package flatbuffers

import (
	"regexp"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazelbuild/bazel-gazelle/language"
)

const (
	languageName = "flatbuffers"
	LibraryKind  = "flatbuffers_library"
)

var SupportedExts = []string{".fbs"}

var include = regexp.MustCompile(`(?m)^\s*include\s+"([^"\n]+)"\s*;`)

// ParseImports returns the sorted, deduplicated paths named by the schema
// statements of one file. Statements inside comments are skipped.
func ParseImports(content []byte) []string {
	return common.ParseIDLStatements(content, include, false)
}

func NewLanguage() language.Language {
	return common.NewIDLLanguage(common.IDLSpec{
		Name:        languageName,
		Kind:        LibraryKind,
		Defs:        "flatbuffers:defs.bzl",
		Exts:        SupportedExts,
		StdPrefix:   "",
		Bindings:    common.IDLBindings("flatbuffers", "library", true),
		AbsoluteDep: false,
	}, ParseImports, "add a flatbuffers_library that owns that file or an exact # gazelle:resolve flatbuffers mapping")
}

func init() { common.RegisterLanguage(NewLanguage) }
