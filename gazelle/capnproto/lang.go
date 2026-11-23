package capnproto

import (
	"regexp"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazelbuild/bazel-gazelle/language"
)

const (
	languageName = "capnproto"
	LibraryKind  = "capnproto_library"
)

var SupportedExts = []string{".capnp"}

var include = regexp.MustCompile(`\bimport\s+"([^"\n]+)"`)

func ParseImports(content []byte) []string {
	return common.ParseIDLStatements(content, include, true)
}

func NewLanguage() language.Language {
	return common.NewIDLLanguage(common.IDLSpec{
		Name:        languageName,
		Kind:        LibraryKind,
		Defs:        "capnproto:defs.bzl",
		Exts:        SupportedExts,
		StdPrefix:   "/capnp/",
		Bindings:    common.IDLBindings("capnproto", "", false),
		AbsoluteDep: true,
	}, ParseImports, "add a capnproto_library that owns that file or an exact # gazelle:resolve capnproto mapping")
}

func init() { common.RegisterLanguage(NewLanguage) }
