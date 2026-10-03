package java

import "github.com/ralvik/rules_dx/gazelle/common"

var SupportedExts = []string{".java"}

var TestSourceSuffixes = []string{"Test"}

const LibraryKind = "java_library"

func IsTestSource(name string) bool {
	return common.IsTestSource(name, SupportedExts, TestSourceSuffixes)
}

func DirTargetName(dir string) (string, error) { return common.DirTargetName(dir) }

func ClassIdentity(name string) string { return common.ClassIdentity(name, SupportedExts) }
