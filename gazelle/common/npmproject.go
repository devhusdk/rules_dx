package common

import (
	"encoding/json"
	"os"
	"path"
	"path/filepath"
	"strings"
)

var npmManifestFile = ManifestName("npm")

func OwningPackageJSON(repoRoot, pkg string) string {
	for dir := pkg; ; dir = path.Dir(dir) {
		if dir == "." {
			dir = ""
		}
		if hasFile(repoRoot, path.Join(dir, npmManifestFile)) {
			return path.Join(dir, npmManifestFile)
		}
		if dir == "" {
			return ""
		}
	}
}

func NpmMissingHint(repoRoot, pkg, name string) string {
	owner := OwningPackageJSON(repoRoot, pkg)
	if owner == "" {
		owner = npmManifestFile
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

func IsESMPackage(repoRoot, pkg string) bool {
	owner := OwningPackageJSON(repoRoot, pkg)
	if owner == "" {
		return false
	}
	p, _ := readPackageJSON(repoRoot, owner)
	return p.Type == "module"
}

func RootDeclares(repoRoot string, names ...string) bool {
	p, ok := readPackageJSON(repoRoot, npmManifestFile)
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

var JestConfigs = []string{"jest.config.js", "jest.config.mjs", "jest.config.cjs", "jest.config.json"}

func JestConfig(repoRoot, pkg string) string {
	for _, name := range JestConfigs {
		if hasFile(repoRoot, path.Join(pkg, name)) {
			return name
		}
	}
	return ""
}

func JestPreflight(repoRoot string) string {
	if RootDeclares(repoRoot, "jest-cli", "jest-junit") {
		return ""
	}
	return `add "jest-cli" and "jest-junit" to devDependencies in ` + npmManifestFile
}

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
