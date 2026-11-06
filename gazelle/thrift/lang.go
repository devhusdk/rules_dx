package thrift

import (
	"regexp"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazelbuild/bazel-gazelle/language"
)

const (
	languageName = "thrift"
	LibraryKind  = "thrift_library"
)

var SupportedExts = []string{".thrift"}

var include = regexp.MustCompile(`(?m)^\s*include\s+(?:"([^"\n]+)"|\'([^\'\n]+)\')`)

// ParseImports returns the sorted, deduplicated paths named by the schema
// statements of one file. Statements inside comments are skipped.
func ParseImports(content []byte) []string {
	return common.ParseIDLStatements(content, include, true)
}

func NewLanguage() language.Language {
	return common.NewIDLLanguage(common.IDLSpec{
		Name:        languageName,
		Kind:        LibraryKind,
		Defs:        "thrift/rules:defs.bzl",
		Exts:        SupportedExts,
		StdPrefix:   "",
		AbsoluteDep: false,
	}, ParseImports, "add a thrift_library that owns that file or an exact # gazelle:resolve thrift mapping")
}

func init() { common.RegisterLanguage(NewLanguage) }
