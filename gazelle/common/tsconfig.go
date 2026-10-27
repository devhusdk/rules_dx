package common

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/tailscale/hujson"
)

// tsPathsKey is the c.Exts key that holds the nearest tsconfig/jsconfig state.
// It is shared by every JavaScript-family language so a config is loaded and
// reported once per directory.
const tsPathsKey = "_dx_tsconfig"

// TSPaths is the module-resolution part of a tsconfig.json or jsconfig.json:
// compilerOptions.baseUrl and compilerOptions.paths, after extends is applied.
// All paths are workspace-relative and slash-separated. A TSPaths is immutable
// once loaded, so languages can share it.
type TSPaths struct {
	// Config is the workspace-relative path of the leaf config file.
	Config  string
	baseURL string
	hasBase bool
	exact   map[string][]string
	wild    []tsPattern
}

type tsPattern struct {
	prefix, suffix string
	targets        []string
}

// tsState is what ConfigureTSPaths stores in c.Exts. rel stamps the directory
// that last configured it, so a second language configuring the same directory
// does not load or report the config again.
type tsState struct {
	rel   string
	paths *TSPaths
}

// ConfigureTSPaths records the nearest tsconfig.json (else jsconfig.json) for
// directory rel in c. A directory without its own config inherits its parent's.
// A directory with its own config uses it, even when it has no paths. The call
// is idempotent per directory: only the call that loads the file returns its
// error. Nearest-config is an approximation; include, files and project
// references are not considered.
func ConfigureTSPaths(c *config.Config, rel string) error {
	var inherited *TSPaths
	if st, ok := c.Exts[tsPathsKey].(*tsState); ok && st != nil {
		if st.rel == rel {
			return nil
		}
		inherited = st.paths
	}
	paths, found, err := LoadTSPaths(c.RepoRoot, rel)
	if err != nil || !found {
		c.Exts[tsPathsKey] = &tsState{rel: rel, paths: inherited}
		return err
	}
	c.Exts[tsPathsKey] = &tsState{rel: rel, paths: paths}
	return nil
}

func tsPathsFor(c *config.Config) *TSPaths {
	if c == nil {
		return nil
	}
	if st, ok := c.Exts[tsPathsKey].(*tsState); ok && st != nil {
		return st.paths
	}
	return nil
}

// tsRawOptions is the part of one config file chain before ${configDir} and
// relative paths are resolved. Dirs are absolute file system paths.
type tsRawOptions struct {
	hasBase  bool
	base     string
	baseDir  string
	hasPaths bool
	paths    map[string][]string
	pathsDir string
}

func (o *tsRawOptions) merge(from tsRawOptions) {
	if from.hasBase {
		o.hasBase, o.base, o.baseDir = true, from.base, from.baseDir
	}
	if from.hasPaths {
		o.hasPaths, o.paths, o.pathsDir = true, from.paths, from.pathsDir
	}
}

// LoadTSPaths reads <rel>/tsconfig.json, else <rel>/jsconfig.json, and applies
// its extends chain. The bool is false when neither file exists.
func LoadTSPaths(repoRoot, rel string) (*TSPaths, bool, error) {
	dir := filepath.Join(repoRoot, filepath.FromSlash(rel))
	var file string
	for _, name := range []string{"tsconfig.json", "jsconfig.json"} {
		candidate := filepath.Join(dir, name)
		if info, err := os.Stat(candidate); err == nil && !info.IsDir() {
			file = candidate
			break
		}
	}
	if file == "" {
		return nil, false, nil
	}
	display := "//" + path.Join(rel, filepath.Base(file))
	raw, err := loadTSConfig(repoRoot, file, map[string]bool{})
	if err != nil {
		return nil, true, fmt.Errorf("tsconfig: %s: %w", display, err)
	}
	return buildTSPaths(repoRoot, dir, raw, display), true, nil
}

func loadTSConfig(repoRoot, file string, stack map[string]bool) (tsRawOptions, error) {
	if stack[file] {
		return tsRawOptions{}, fmt.Errorf("extends cycle through %s", relDisplay(repoRoot, file))
	}
	stack[file] = true
	defer delete(stack, file)
	data, err := os.ReadFile(file)
	if err != nil {
		return tsRawOptions{}, err
	}
	var doc struct {
		Extends         json.RawMessage `json:"extends"`
		CompilerOptions struct {
			BaseURL *string             `json:"baseUrl"`
			Paths   map[string][]string `json:"paths"`
		} `json:"compilerOptions"`
	}
	if err := unmarshalJSONC(data, &doc); err != nil {
		return tsRawOptions{}, fmt.Errorf("%s: %v", relDisplay(repoRoot, file), err)
	}
	var extends []string
	if len(doc.Extends) > 0 && string(doc.Extends) != "null" {
		var one string
		if err := json.Unmarshal(doc.Extends, &one); err == nil {
			extends = []string{one}
		} else if err := json.Unmarshal(doc.Extends, &extends); err != nil {
			return tsRawOptions{}, fmt.Errorf("%s: extends must be a string or an array of strings", relDisplay(repoRoot, file))
		}
	}
	dir := filepath.Dir(file)
	var out tsRawOptions
	for _, ext := range extends {
		parent, ok, err := resolveExtends(repoRoot, dir, ext)
		if err != nil {
			return tsRawOptions{}, fmt.Errorf("%s: %v", relDisplay(repoRoot, file), err)
		}
		if !ok {
			continue
		}
		opts, err := loadTSConfig(repoRoot, parent, stack)
		if err != nil {
			return tsRawOptions{}, err
		}
		out.merge(opts)
	}
	var own tsRawOptions
	if doc.CompilerOptions.BaseURL != nil {
		own.hasBase, own.base, own.baseDir = true, *doc.CompilerOptions.BaseURL, dir
	}
	if doc.CompilerOptions.Paths != nil {
		own.hasPaths, own.paths, own.pathsDir = true, doc.CompilerOptions.Paths, dir
	}
	out.merge(own)
	return out, nil
}

// resolveExtends finds the file an extends entry names. A relative or absolute
// entry that does not exist is an error. A package entry is looked up in
// node_modules from dir up to the repo root, and skipped when not found.
func resolveExtends(repoRoot, dir, ext string) (string, bool, error) {
	if ext == "" {
		return "", false, errors.New("empty extends entry")
	}
	if strings.HasPrefix(ext, "./") || strings.HasPrefix(ext, "../") || ext == "." || ext == ".." || filepath.IsAbs(ext) {
		base := ext
		if !filepath.IsAbs(base) {
			base = filepath.Join(dir, filepath.FromSlash(ext))
		}
		if file, ok := existingConfigFile(base); ok {
			return file, true, nil
		}
		return "", false, fmt.Errorf("extends %q: file not found", ext)
	}
	root := filepath.Clean(repoRoot)
	for d := dir; ; d = filepath.Dir(d) {
		if file, ok := existingConfigFile(filepath.Join(d, "node_modules", filepath.FromSlash(ext))); ok {
			return file, true, nil
		}
		if d == root || filepath.Dir(d) == d || !strings.HasPrefix(d, root) {
			break
		}
	}
	return "", false, nil
}

func existingConfigFile(base string) (string, bool) {
	isFile := func(p string) bool {
		info, err := os.Stat(p)
		return err == nil && !info.IsDir()
	}
	if isFile(base) {
		return base, true
	}
	if !strings.HasSuffix(base, ".json") && isFile(base+".json") {
		return base + ".json", true
	}
	if info, err := os.Stat(base); err == nil && info.IsDir() && isFile(filepath.Join(base, "tsconfig.json")) {
		return filepath.Join(base, "tsconfig.json"), true
	}
	return "", false
}

func relDisplay(repoRoot, file string) string {
	if r, ok := workspaceRel(repoRoot, file); ok {
		return "//" + r
	}
	return file
}

// workspaceRel turns an absolute path into a workspace-relative slash path. It
// reports false when the path leaves the workspace.
func workspaceRel(repoRoot, abs string) (string, bool) {
	r, err := filepath.Rel(filepath.Clean(repoRoot), filepath.Clean(abs))
	if err != nil {
		return "", false
	}
	r = filepath.ToSlash(r)
	if r == ".." || strings.HasPrefix(r, "../") {
		return "", false
	}
	if r == "." {
		r = ""
	}
	return r, true
}

// resolveTSPath resolves a baseUrl or paths target against dir, substituting
// ${configDir} with the leaf config's directory.
func resolveTSPath(repoRoot, dir, leafDir, p string) (string, bool) {
	return workspaceRel(repoRoot, resolveAbs(dir, leafDir, p))
}

func buildTSPaths(repoRoot, leafDir string, raw tsRawOptions, display string) *TSPaths {
	out := &TSPaths{Config: strings.TrimPrefix(display, "//"), exact: map[string][]string{}}
	pathsDir := raw.pathsDir
	if raw.hasBase {
		if base, ok := resolveTSPath(repoRoot, raw.baseDir, leafDir, raw.base); ok {
			out.baseURL, out.hasBase = base, true
		}
		pathsDir = resolveAbs(raw.baseDir, leafDir, raw.base)
	}
	if !raw.hasPaths {
		return out
	}
	keys := make([]string, 0, len(raw.paths))
	for k := range raw.paths {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	for _, key := range keys {
		var targets []string
		for _, t := range raw.paths[key] {
			if r, ok := resolveTSPath(repoRoot, pathsDir, leafDir, t); ok {
				targets = append(targets, r)
			}
		}
		if len(targets) == 0 {
			continue
		}
		switch strings.Count(key, "*") {
		case 0:
			out.exact[key] = targets
		case 1:
			i := strings.Index(key, "*")
			out.wild = append(out.wild, tsPattern{prefix: key[:i], suffix: key[i+1:], targets: targets})
		}
	}
	return out
}

func resolveAbs(dir, leafDir, p string) string {
	if rest, ok := strings.CutPrefix(p, "${configDir}"); ok {
		return filepath.Clean(leafDir + rest)
	}
	if filepath.IsAbs(p) {
		return filepath.Clean(p)
	}
	return filepath.Join(dir, filepath.FromSlash(p))
}

// Candidates returns the workspace-relative paths a bare import spec may name,
// in TypeScript's order: the exact paths key, else the wildcard key with the
// longest prefix (all its targets in order), then baseUrl/<spec>. Extensions
// are left on. Paths that leave the workspace are dropped.
func (t *TSPaths) Candidates(spec string) []string {
	if t == nil || spec == "" {
		return nil
	}
	var raw []string
	if targets, ok := t.exact[spec]; ok {
		raw = append(raw, targets...)
	} else {
		best := -1
		for i, p := range t.wild {
			if len(spec) < len(p.prefix)+len(p.suffix) || !strings.HasPrefix(spec, p.prefix) || !strings.HasSuffix(spec, p.suffix) {
				continue
			}
			if best < 0 || len(p.prefix) > len(t.wild[best].prefix) {
				best = i
			}
		}
		if best >= 0 {
			p := t.wild[best]
			star := spec[len(p.prefix) : len(spec)-len(p.suffix)]
			for _, target := range p.targets {
				raw = append(raw, strings.Replace(target, "*", star, 1))
			}
		}
	}
	if t.hasBase {
		raw = append(raw, path.Join(t.baseURL, spec))
	}
	seen := map[string]bool{}
	var out []string
	for _, r := range raw {
		r = path.Clean(r)
		if r == "." || r == ".." || strings.HasPrefix(r, "../") || strings.HasPrefix(r, "/") || seen[r] {
			continue
		}
		seen[r] = true
		out = append(out, r)
	}
	return out
}

// unmarshalJSONC decodes JSON with comments and trailing commas (as used by
// tsconfig files) into v.
func unmarshalJSONC(data []byte, v any) error {
	std, err := hujson.Standardize(data)
	if err != nil {
		return err
	}
	return json.Unmarshal(std, v)
}
