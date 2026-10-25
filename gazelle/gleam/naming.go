package gleam

import (
	"bufio"
	"bytes"
	"os"
	"path/filepath"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

const (
	LibraryKind     = "gleam_library"
	TestKind        = "gleam_test"
	TestLibraryKind = "gleam_test_library"
	ManifestFile    = "gleam.toml"
	SourceDir       = "src"
	TestDir         = "test"
	Extension       = ".gleam"
)

func DirTargetName(dir string) (string, error) { return common.DirTargetName(dir) }

func rootPackageName(dir string) string {
	content, err := os.ReadFile(filepath.Join(dir, ManifestFile))
	if err != nil {
		return ""
	}
	return manifestName(content)
}

func manifestName(content []byte) string {
	scanner := bufio.NewScanner(bytes.NewReader(content))
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if strings.HasPrefix(line, "[") {
			return ""
		}
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		key, value, ok := strings.Cut(line, "=")
		if !ok || strings.TrimSpace(key) != "name" {
			continue
		}
		value = strings.TrimSpace(value)
		if len(value) < 2 || (value[0] != '"' && value[0] != '\'') {
			return ""
		}
		end := strings.IndexByte(value[1:], value[0])
		if end < 0 {
			return ""
		}
		return value[1 : end+1]
	}
	return ""
}

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

func IsTestHelper(path string) bool {
	return strings.HasPrefix(path, TestDir+"/") && strings.HasSuffix(path, Extension) && !IsTestSource(path)
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
