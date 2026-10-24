package kotlin

import "github.com/devhusdk/rules_dx/gazelle/common"

var SupportedExts = []string{".kt"}

var TestSourceSuffixes = []string{"Test"}

const LibraryKind = "kotlin_library"

func IsTestSource(name string) bool {
	return common.IsTestSource(name, SupportedExts, TestSourceSuffixes)
}

func DirTargetName(dir string) (string, error) { return common.DirTargetName(dir) }
