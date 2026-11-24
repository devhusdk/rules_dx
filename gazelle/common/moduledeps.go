package common

import (
	"sync"

	"github.com/bazelbuild/buildtools/build"
)

var moduleDeps sync.Map

func ModuleDeps(repoRoot string) map[string]bool {
	if deps, ok := moduleDeps.Load(repoRoot); ok {
		return deps.(map[string]bool)
	}
	deps, _ := moduleDeps.LoadOrStore(repoRoot, readModuleDeps(repoRoot))
	return deps.(map[string]bool)
}

func readModuleDeps(repoRoot string) map[string]bool {
	deps := map[string]bool{}
	for _, call := range moduleCalls(repoRoot) {
		if fn, ok := call.X.(*build.Ident); !ok || fn.Name != "bazel_dep" || hasStarStar(call) {
			continue
		}
		if name, ok := stringArg(call, "name"); ok {
			deps[name] = true
		}
	}
	return deps
}
