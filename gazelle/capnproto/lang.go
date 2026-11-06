package capnproto

import (
	"regexp"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazelbuild/bazel-gazelle/language"
)

const (
	languageName = "capnproto"
	LibraryKind  = "capnp_library"
)

var SupportedExts = []string{".capnp"}

var include = regexp.MustCompile(`\bimport\s+"([^"\n]+)"`)

// ParseImports returns the sorted, deduplicated paths named by the schema
// statements of one file. Statements inside comments are skipped.
func ParseImports(content []byte) []string {
	return common.ParseIDLStatements(content, include, true)
}

func NewLanguage() language.Language {
	return common.NewIDLLanguage(common.IDLSpec{
		Name:        languageName,
		Kind:        LibraryKind,
		Defs:        "capnproto/rules:defs.bzl",
		Exts:        SupportedExts,
		StdPrefix:   "/capnp/",
		AbsoluteDep: true,
	}, ParseImports, "add a capnp_library that owns that file or an exact # gazelle:resolve capnproto mapping")
}

func init() { common.RegisterLanguage(NewLanguage) }
