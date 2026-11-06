package common

import (
	"encoding/json"
	"os"
	"path"
	"path/filepath"
	"strings"
)

// OwningPackageJSON returns the repo-relative path of the package.json nearest
// to pkg, at or above it, or "" when there is none.
func OwningPackageJSON(repoRoot, pkg string) string {
	for dir := pkg; ; dir = path.Dir(dir) {
		if dir == "." {
			dir = ""
		}
		if hasFile(repoRoot, path.Join(dir, "package.json")) {
			return path.Join(dir, "package.json")
		}
		if dir == "" {
			return ""
		}
	}
}

// NpmMissingHint tells the user which package.json should declare the
// package a bare import names.
func NpmMissingHint(repoRoot, pkg, name string) string {
	owner := OwningPackageJSON(repoRoot, pkg)
	if owner == "" {
		owner = "package.json"
	}
	return "add \"" + NpmPackageName(name) + "\" to " + owner + " dependencies"
}

type packageJSON struct {
	Type            string                     `json:"type"`
	Dependencies    map[string]json.RawMessage `json:"dependencies"`
	DevDependencies map[string]json.RawMessage `json:"devDependencies"`
}

func readPackageJSON(repoRoot, rel string) (packageJSON, bool) {
	var pkg packageJSON
	data, err := os.ReadFile(filepath.Join(repoRoot, filepath.FromSlash(rel)))
	if err != nil || json.Unmarshal(data, &pkg) != nil {
		return pkg, false
	}
	return pkg, true
}

// IsESMPackage reports whether the package.json owning pkg sets "type": "module".
func IsESMPackage(repoRoot, pkg string) bool {
	owner := OwningPackageJSON(repoRoot, pkg)
	if owner == "" {
		return false
	}
	p, _ := readPackageJSON(repoRoot, owner)
	return p.Type == "module"
}

// RootDeclares reports whether the root package.json declares every name in
// dependencies or devDependencies.
func RootDeclares(repoRoot string, names ...string) bool {
	p, ok := readPackageJSON(repoRoot, "package.json")
	if !ok {
		return false
	}
	for _, name := range names {
		_, dep := p.Dependencies[name]
		_, dev := p.DevDependencies[name]
		if !dep && !dev {
			return false
		}
	}
	return true
}

// JestPreflight returns the message for a root package.json that cannot run
// generated jest tests, or "".
func JestPreflight(repoRoot string) string {
	if RootDeclares(repoRoot, "jest-cli", "jest-junit") {
		return ""
	}
	return `add "jest-cli" and "jest-junit" to devDependencies in package.json`
}

// NearestTSConfig returns the label of the ts_config target in the nearest
// package at or above rel that holds tsconfig.json, relative to rel, or "".
func NearestTSConfig(repoRoot, rel string) string {
	for dir := rel; ; dir = path.Dir(dir) {
		if dir == "." {
			dir = ""
		}
		if hasFile(repoRoot, path.Join(dir, "tsconfig.json")) {
			if dir == rel {
				return ":tsconfig"
			}
			return "//" + dir + ":tsconfig"
		}
		if dir == "" {
			return ""
		}
	}
}

// TSConfigParents returns the labels of the workspace files rel/tsconfig.json
// extends by relative path. A parent named tsconfig.json is its package's
// ts_config target; other files are named directly.
func TSConfigParents(repoRoot, rel string) ([]string, error) {
	data, err := os.ReadFile(filepath.Join(repoRoot, filepath.FromSlash(rel), "tsconfig.json"))
	if err != nil {
		return nil, err
	}
	var raw struct {
		Extends json.RawMessage `json:"extends"`
	}
	if err := unmarshalJSONC(data, &raw); err != nil {
		return nil, err
	}
	var list []string
	var one string
	if json.Unmarshal(raw.Extends, &one) == nil {
		list = []string{one}
	} else {
		_ = json.Unmarshal(raw.Extends, &list)
	}
	var out []string
	for _, ext := range list {
		if !strings.HasPrefix(ext, "./") && !strings.HasPrefix(ext, "../") {
			continue
		}
		abs, ok := existingConfigFile(filepath.Join(repoRoot, filepath.FromSlash(rel), filepath.FromSlash(ext)))
		if !ok {
			continue
		}
		file, ok := workspaceRel(repoRoot, abs)
		if !ok {
			continue
		}
		dir, base := path.Dir(file), path.Base(file)
		if dir == "." {
			dir = ""
		}
		name := base
		if base == "tsconfig.json" {
			name = "tsconfig"
		}
		if dir == rel {
			out = append(out, ":"+name)
		} else {
			out = append(out, "//"+dir+":"+name)
		}
	}
	return out, nil
}
