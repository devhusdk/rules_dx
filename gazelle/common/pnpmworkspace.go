package common

import (
	"encoding/json"
	"os"
	"path"
	"path/filepath"
	"strings"
)

// IsPnpmWorkspaceMember reports whether rel holds a package.json and is listed as
// a workspace package by an ancestor directory that holds pnpm-lock.yaml.
func IsPnpmWorkspaceMember(repoRoot, rel string) bool {
	if rel == "" || repoRoot == "" || !hasFile(repoRoot, path.Join(rel, "package.json")) {
		return false
	}
	for dir := path.Dir(rel); ; dir = path.Dir(dir) {
		if dir == "." {
			dir = ""
		}
		abs := filepath.Join(repoRoot, filepath.FromSlash(dir))
		if hasFile(repoRoot, path.Join(dir, "pnpm-lock.yaml")) {
			sub := strings.TrimPrefix(strings.TrimPrefix(rel, dir), "/")
			if matchesWorkspace(workspacePatterns(abs), sub) {
				return true
			}
		}
		if dir == "" {
			return false
		}
	}
}

func workspacePatterns(dir string) []string {
	var patterns []string
	if data, err := os.ReadFile(filepath.Join(dir, "pnpm-workspace.yaml")); err == nil {
		inPackages := false
		for _, line := range strings.Split(string(data), "\n") {
			trimmed := strings.TrimSpace(line)
			switch {
			case strings.HasPrefix(line, "packages:"):
				inPackages = true
			case inPackages && strings.HasPrefix(trimmed, "- "):
				patterns = append(patterns, strings.Trim(strings.TrimSpace(trimmed[2:]), `'"`))
			case inPackages && trimmed != "" && !strings.HasPrefix(trimmed, "#") && !strings.HasPrefix(line, " "):
				inPackages = false
			}
		}
	}
	if data, err := os.ReadFile(filepath.Join(dir, "package.json")); err == nil {
		var pkg struct {
			Workspaces json.RawMessage `json:"workspaces"`
		}
		if json.Unmarshal(data, &pkg) == nil {
			var list []string
			var obj struct {
				Packages []string `json:"packages"`
			}
			if json.Unmarshal(pkg.Workspaces, &list) == nil {
				patterns = append(patterns, list...)
			} else if json.Unmarshal(pkg.Workspaces, &obj) == nil {
				patterns = append(patterns, obj.Packages...)
			}
		}
	}
	return patterns
}

func matchesWorkspace(patterns []string, rel string) bool {
	matched := false
	for _, p := range patterns {
		negate := strings.HasPrefix(p, "!")
		p = strings.Trim(strings.TrimPrefix(p, "!"), "/")
		p = strings.TrimPrefix(p, "./")
		if matchSegments(strings.Split(p, "/"), strings.Split(rel, "/")) {
			matched = !negate
		}
	}
	return matched
}

func matchSegments(pat, segs []string) bool {
	if len(pat) == 0 {
		return len(segs) == 0
	}
	if pat[0] == "**" {
		for i := 0; i <= len(segs); i++ {
			if matchSegments(pat[1:], segs[i:]) {
				return true
			}
		}
		return false
	}
	if len(segs) == 0 {
		return false
	}
	if ok, _ := path.Match(pat[0], segs[0]); !ok {
		return false
	}
	return matchSegments(pat[1:], segs[1:])
}
