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

func TestTargetName(src string) (string, error) {
	return common.Normalize(strings.TrimSuffix(path.Base(src), sourceExt))
}

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
