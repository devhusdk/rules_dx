package avro

import (
	"regexp"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazelbuild/bazel-gazelle/language"
)

const (
	languageName = "avro"
	LibraryKind  = "avro_library"
)

var SupportedExts = []string{".avsc", ".avdl"}

var include = regexp.MustCompile(`(?m)^\s*import\s+(?:idl|schema)\s+"([^"\n]+)"\s*;`)

// ParseImports returns the sorted, deduplicated paths named by the schema
// statements of one file. Statements inside comments are skipped.
func ParseImports(content []byte) []string {
	return common.ParseIDLStatements(content, include, false)
}

func NewLanguage() language.Language {
	return common.NewIDLLanguage(common.IDLSpec{
		Name:        languageName,
		Kind:        LibraryKind,
		Defs:        "avro:defs.bzl",
		Exts:        SupportedExts,
		StdPrefix:   "",
		Bindings:    common.IDLBindings("avro", "", false),
		AbsoluteDep: false,
	}, ParseImports, "add an avro_library that owns that file or an exact # gazelle:resolve avro mapping")
}

func init() { common.RegisterLanguage(NewLanguage) }
