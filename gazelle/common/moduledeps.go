package common

import (
	"os"
	"path/filepath"
	"sync"

	"github.com/bazelbuild/buildtools/build"
)

var moduleDeps sync.Map

// ModuleDeps returns the bazel_dep module names declared in the root
// MODULE.bazel. It is empty when the file is missing or unparsable.
func ModuleDeps(repoRoot string) map[string]bool {
	if deps, ok := moduleDeps.Load(repoRoot); ok {
		return deps.(map[string]bool)
	}
	deps, _ := moduleDeps.LoadOrStore(repoRoot, readModuleDeps(repoRoot))
	return deps.(map[string]bool)
}

func readModuleDeps(repoRoot string) map[string]bool {
	deps := map[string]bool{}
	data, err := os.ReadFile(filepath.Join(repoRoot, "MODULE.bazel"))
	if err != nil {
		return deps
	}
	file, err := build.ParseModule("MODULE.bazel", data)
	if err != nil {
		return deps
	}
	for _, stmt := range file.Stmt {
		call, ok := stmt.(*build.CallExpr)
		if !ok {
			continue
		}
		if fn, ok := call.X.(*build.Ident); !ok || fn.Name != "bazel_dep" || hasStarStar(call) {
			continue
		}
		if name, ok := stringArg(call, "name"); ok {
			deps[name] = true
		}
	}
	return deps
}
