package common

import (
	"os"
	"path"
	"path/filepath"
	"strings"

	"github.com/bazelbuild/buildtools/build"
)

// moduleCalls returns the top-level calls of the root MODULE.bazel and of every
// file it includes, in source order. Includes that are not workspace labels
// ending in .MODULE.bazel are ignored, as are unreadable or unparsable files.
func moduleCalls(repoRoot string) []*build.CallExpr {
	var calls []*build.CallExpr
	seen := map[string]bool{}
	var walk func(rel string)
	walk = func(rel string) {
		if seen[rel] {
			return
		}
		seen[rel] = true
		data, err := os.ReadFile(filepath.Join(repoRoot, filepath.FromSlash(rel)))
		if err != nil {
			return
		}
		file, err := build.ParseModule(rel, data)
		if err != nil {
			return
		}
		for _, stmt := range file.Stmt {
			call, ok := stmt.(*build.CallExpr)
			if !ok {
				continue
			}
			if fn, ok := call.X.(*build.Ident); ok && fn.Name == "include" {
				if hasStarStar(call) {
					continue
				}
				if label, ok := includeLabel(call); ok {
					walk(repoRelative(label))
				}
				continue
			}
			calls = append(calls, call)
		}
	}
	walk("MODULE.bazel")
	return calls
}

func includeLabel(call *build.CallExpr) (string, bool) {
	var label string
	var ok bool
	if len(call.List) == 1 {
		label, ok = stringValue(call.List[0])
	}
	if !ok {
		label, ok = stringArg(call, "label")
	}
	if !ok || !strings.HasPrefix(label, "//") || !strings.HasSuffix(label, ".MODULE.bazel") {
		return "", false
	}
	rel := repoRelative(label)
	if rel == ".." || strings.HasPrefix(rel, "../") || path.IsAbs(rel) {
		return "", false
	}
	return label, true
}
