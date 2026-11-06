package common

import (
	"path"
	"strings"
	"sync"

	"github.com/bazelbuild/buildtools/build"
)

const defaultNpmHub = "npm"

var npmHubs sync.Map

// NpmHub returns the repository name of the npm hub that links the
// pnpm-lock.yaml at lockRel, read from the npm_translate_lock call in the root
// MODULE.bazel and the files it includes. It returns "npm" when nothing matches.
func NpmHub(repoRoot, lockRel string) string {
	hubs, ok := npmHubs.Load(repoRoot)
	if !ok {
		hubs, _ = npmHubs.LoadOrStore(repoRoot, readNpmHubs(repoRoot))
	}
	if hub, ok := hubs.(map[string]string)[path.Clean(lockRel)]; ok {
		return hub
	}
	return defaultNpmHub
}

func readNpmHubs(repoRoot string) map[string]string {
	hubs := map[string]string{}
	aliases := map[string]string{}
	type lock struct{ name, lock string }
	var locks []lock
	for _, call := range moduleCalls(repoRoot) {
		switch fn := call.X.(type) {
		case *build.Ident:
			if fn.Name == "use_repo" {
				for _, arg := range call.List {
					if assign, ok := arg.(*build.AssignExpr); ok {
						lhs, _ := assign.LHS.(*build.Ident)
						if name, ok := stringValue(assign.RHS); ok && lhs != nil {
							aliases[name] = lhs.Name
						}
					}
				}
			}
		case *build.DotExpr:
			if fn.Name != "npm_translate_lock" || hasStarStar(call) {
				continue
			}
			name, ok1 := stringArg(call, "name")
			pnpm, ok2 := stringArg(call, "pnpm_lock")
			if ok1 && ok2 {
				locks = append(locks, lock{name, repoRelative(pnpm)})
			}
		}
	}
	for _, l := range locks {
		hub := l.name
		if alias, ok := aliases[l.name]; ok {
			hub = alias
		}
		hubs[l.lock] = hub
	}
	return hubs
}

func hasStarStar(call *build.CallExpr) bool {
	for _, arg := range call.List {
		if u, ok := arg.(*build.UnaryExpr); ok && u.Op == "**" {
			return true
		}
	}
	return false
}

func stringValue(e build.Expr) (string, bool) {
	s, ok := e.(*build.StringExpr)
	if !ok {
		return "", false
	}
	return s.Value, true
}

func stringArg(call *build.CallExpr, key string) (string, bool) {
	for _, arg := range call.List {
		if assign, ok := arg.(*build.AssignExpr); ok {
			if lhs, ok := assign.LHS.(*build.Ident); ok && lhs.Name == key {
				return stringValue(assign.RHS)
			}
		}
	}
	return "", false
}

func repoRelative(label string) string {
	label = strings.TrimPrefix(strings.TrimPrefix(label, "@@"), "@")
	label = strings.TrimPrefix(label, "//")
	pkg, name, ok := strings.Cut(label, ":")
	if !ok {
		return path.Clean(label)
	}
	return path.Clean(path.Join(pkg, name))
}
