package fsharp

import "github.com/ralvik/rules_dx/gazelle/common"

var SupportedExts = []string{".fs"}

var TestSourceSuffixes = []string{"Test"}

const LibraryKind = "fsharp_library"

func IsTestSource(name string) bool {
	return common.IsTestSource(name, SupportedExts, TestSourceSuffixes)
}

func DirTargetName(dir string) (string, error) { return common.DirTargetName(dir) }

func ClassIdentity(name string) string { return common.ClassIdentity(name, SupportedExts) }
