package python

import (
	"os"
	"path/filepath"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/devhusdk/rules_dx/gazelle/common"
	"github.com/pelletier/go-toml/v2"
)

const (
	ruffKind   = "ruff_config"
	ruffName   = "ruff_config"
	ruffExt    = "python_ruff"
	nativeDefs = "quality:native_config.bzl"
)

// ruffState is the nearest ruff config at or above a directory.
type ruffState struct {
	rel  string
	file string
}

func (s *ruffState) label() string { return "//" + s.rel + ":" + ruffName }

func ruffFile(dir string) string {
	for _, name := range []string{".ruff.toml", "ruff.toml"} {
		if _, err := os.Stat(filepath.Join(dir, name)); err == nil {
			return name
		}
	}
	content, err := os.ReadFile(filepath.Join(dir, "pyproject.toml"))
	if err != nil {
		return ""
	}
	var raw struct {
		Tool map[string]any `toml:"tool"`
	}
	if toml.Unmarshal(common.StripBOM(content), &raw) != nil {
		return ""
	}
	if _, ok := raw.Tool["ruff"]; ok {
		return "pyproject.toml"
	}
	return ""
}

func configureRuff(c *config.Config, rel string, _ *rule.File, _ common.Reporter) {
	if file := ruffFile(filepath.Join(c.RepoRoot, filepath.FromSlash(rel))); file != "" {
		c.Exts[ruffExt] = &ruffState{rel: rel, file: file}
	}
}

func ruffAt(c *config.Config) *ruffState {
	if c == nil {
		return nil
	}
	state, _ := c.Exts[ruffExt].(*ruffState)
	return state
}

func ruffRule(state *ruffState, rel string) *rule.Rule {
	r := rule.NewRule(ruffKind, ruffName)
	r.SetAttr("src", state.file)
	r.SetAttr("visibility", []string{"//" + rel + ":__subpackages__"})
	return r
}

// withRuffHint returns the user's aspect_hints plus the ruff config label.
func withRuffHint(existing *rule.Rule, state *ruffState, rel string) []string {
	var hints []string
	if existing != nil {
		hints = existing.AttrStrings("aspect_hints")
	}
	if state == nil {
		return hints
	}
	label := state.label()
	if state.rel == rel {
		label = ":" + ruffName
	}
	for _, hint := range hints {
		if hint == label || hint == state.label() {
			return hints
		}
	}
	return append(hints, label)
}

func existingRule(file *rule.File, kind, name string) *rule.Rule {
	if file == nil {
		return nil
	}
	for _, r := range file.Rules {
		if r.Name() == name && r.Kind() == kind {
			return r
		}
	}
	return nil
}

