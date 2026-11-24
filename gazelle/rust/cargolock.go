package rust

import (
	"encoding/json"
	"os"
	"path"
	"path/filepath"
)

const cargoHubLockName = "cargo-bazel-lock.json"

func hubLibNames(repoRoot, rel string) map[string]string {
	for dir := rel; ; dir = path.Dir(dir) {
		if dir == "." {
			dir = ""
		}
		content, err := os.ReadFile(filepath.Join(repoRoot, filepath.FromSlash(dir), cargoHubLockName))
		if err == nil {
			return parseHubLibNames(content)
		}
		if dir == "" {
			return nil
		}
	}
}

func parseHubLibNames(content []byte) map[string]string {
	var lock struct {
		Crates map[string]struct {
			Name              string `json:"name"`
			LibraryTargetName string `json:"library_target_name"`
		} `json:"crates"`
	}
	if err := json.Unmarshal(content, &lock); err != nil {
		return nil
	}
	names := make(map[string]string)
	ambiguous := make(map[string]bool)
	for _, c := range lock.Crates {
		if c.Name == "" || c.LibraryTargetName == "" {
			continue
		}
		if prev, ok := names[c.Name]; ok && prev != c.LibraryTargetName {
			ambiguous[c.Name] = true
		}
		names[c.Name] = c.LibraryTargetName
	}
	for name := range ambiguous {
		delete(names, name)
	}
	return names
}

func (m *cargoManifest) applyHubLibNames(libNames map[string]string) {
	if len(libNames) == 0 {
		return
	}
	for _, depMap := range []map[string]cargoDependency{m.normalDeps, m.devDeps, m.buildDeps} {
		for key, dep := range depMap {
			if !dep.external || dep.renamed || dep.workspace {
				continue
			}
			lib, ok := libNames[dep.label]
			if !ok {
				continue
			}
			if crate, err := CrateName(key); err != nil || crate == lib {
				continue
			}
			if _, taken := depMap[lib]; taken {
				continue
			}
			delete(depMap, key)
			depMap[lib] = dep
		}
	}
}
