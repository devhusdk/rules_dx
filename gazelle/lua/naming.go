package lua

import (
	"path"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

var SupportedExts = []string{".lua"}

const (
	LibraryKind = "lua_library"
	TestKind    = "lua_test"
	BinaryKind  = "lua_binary"
	sourceExt   = ".lua"
	testSuffix  = "_test.lua"
)

func DirTargetName(dir string) (string, error) { return common.DirTargetName(dir) }

// TestTargetName names the lua_test of a *_test.lua source.
func TestTargetName(src string) (string, error) {
	return common.Normalize(strings.TrimSuffix(path.Base(src), sourceExt))
}

// IsTestSource reports whether name is a *_test.lua file.
func IsTestSource(name string) bool { return strings.HasSuffix(name, testSuffix) }

func trimSlashes(p string) string { return strings.Trim(strings.TrimSpace(p), "/") }

func importRoot(pkg, stripImportPrefix string) string {
	if stripImportPrefix == "" {
		return ""
	}
	if strings.HasPrefix(stripImportPrefix, "/") {
		return trimSlashes(stripImportPrefix[1:])
	}
	var parts []string
	for _, p := range []string{pkg, stripImportPrefix} {
		if p != "" {
			parts = append(parts, p)
		}
	}
	return trimSlashes(strings.Join(parts, "/"))
}

// ModuleName returns the require name of the repo-relative .lua path
// path.Join(pkg, src) under the rule's strip_import_prefix and import_prefix.
// It ports module_name in lua/private/lua.bzl; keep the two in sync with the
// vectors in naming_test.go and lua/rules/wrapper_tests.bzl.
func ModuleName(pkg, src, stripImportPrefix, importPrefix string) (string, bool) {
	return moduleNameOf(path.Join(pkg, src), pkg, stripImportPrefix, importPrefix)
}

func moduleNameOf(full, pkg, stripImportPrefix, importPrefix string) (string, bool) {
	if !strings.HasSuffix(full, sourceExt) {
		return "", false
	}
	rest := full
	if root := importRoot(pkg, stripImportPrefix); root != "" {
		if !strings.HasPrefix(full, root+"/") {
			return "", false
		}
		rest = full[len(root)+1:]
	}
	segments := strings.Split(strings.TrimSuffix(rest, sourceExt), "/")
	var prefix []string
	for _, s := range strings.Split(strings.ReplaceAll(importPrefix, ".", "/"), "/") {
		if s != "" {
			prefix = append(prefix, s)
		}
	}
	if segments[len(segments)-1] == "init" && (len(segments) > 1 || len(prefix) > 0) {
		segments = segments[:len(segments)-1]
	}
	segments = append(prefix, segments...)
	for _, s := range segments {
		if s == "" || strings.Contains(s, ".") {
			return "", false
		}
	}
	return strings.Join(segments, "."), true
}
