package gleam

import (
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

const (
	LibraryKind  = "gleam_library"
	TestKind     = "gleam_test"
	ManifestFile = "gleam.toml"
	SourceDir    = "src"
	TestDir      = "test"
	Extension    = ".gleam"
)

func DirTargetName(dir string) (string, error) { return common.DirTargetName(dir) }

func ModuleName(path string) string {
	path = strings.TrimSuffix(path, Extension)
	path = strings.TrimPrefix(path, SourceDir+"/")
	return strings.TrimPrefix(path, TestDir+"/")
}

func IsLibrarySource(path string) bool {
	return strings.HasPrefix(path, SourceDir+"/") && strings.HasSuffix(path, Extension)
}

func IsTestSource(path string) bool {
	return strings.HasPrefix(path, TestDir+"/") && strings.HasSuffix(path, "_test"+Extension)
}

func IsStdLib(name string) bool {
	return name == "gleeunit" || strings.HasPrefix(name, "gleeunit/")
}

func Candidates(name string) []string {
	if strings.HasPrefix(name, "gleam/") {
		return []string{"gleam_stdlib"}
	}
	return nil
}
