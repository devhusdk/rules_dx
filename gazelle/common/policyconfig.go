package common

import (
	"fmt"
	"path/filepath"
	"strings"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

// PolicyConfigDirective binds a tool to the config target the root
// dx_generated_policy lists in configs:
//
//	# gazelle:dx_policy_config <tool> <label>
//
// Packages below inherit the binding. Generated targets then skip the
// aspect_hint for that config.
const PolicyConfigDirective = "dx_policy_config"

type policyConfigs map[string]string

// ConfigurePolicyConfig reads the directive in f on top of what the parent
// directory bound.
func ConfigurePolicyConfig(c *config.Config, f *rule.File) []string {
	var errs []string
	bound := policyConfigs{}
	if parent, ok := c.Exts[PolicyConfigDirective].(policyConfigs); ok {
		for tool, label := range parent {
			bound[tool] = label
		}
	}
	if f != nil {
		for _, d := range f.Directives {
			if d.Key != PolicyConfigDirective {
				continue
			}
			fields := strings.Fields(d.Value)
			if len(fields) != 2 {
				errs = append(errs, fmt.Sprintf("%s wants <tool> <label>, got %q", PolicyConfigDirective, d.Value))
				continue
			}
			bound[fields[0]] = fields[1]
		}
	}
	c.Exts[PolicyConfigDirective] = bound
	return errs
}

// PolicyBinds reports whether the policy binds tool to label, an absolute
// label such as //pkg:ruff_config.
func PolicyBinds(c *config.Config, tool, label string) bool {
	bound, _ := c.Exts[PolicyConfigDirective].(policyConfigs)
	want, ok := bound[tool]
	if !ok {
		return false
	}
	return normalizePolicyLabel(want) == normalizePolicyLabel(label)
}

// InitOwnedConfigs maps each tool to the src of the <tool>_config rule in
// .dx/config/BUILD.bazel (dx init) or .dx/BUILD.bazel (dx generate), the
// second winning, and returns nil when neither file parses.
func InitOwnedConfigs(repoRoot string) map[string]string {
	var owned map[string]string
	for _, pkg := range []string{".dx/config", ".dx"} {
		f, err := rule.LoadFile(filepath.Join(repoRoot, filepath.FromSlash(pkg), "BUILD.bazel"), pkg)
		if err != nil {
			continue
		}
		if owned == nil {
			owned = map[string]string{}
		}
		for _, r := range f.Rules {
			tool, ok := strings.CutSuffix(r.Kind(), "_config")
			if !ok || tool == "" {
				continue
			}
			if src := r.AttrString("src"); src != "" {
				owned[tool] = src
			}
		}
	}
	return owned
}

func normalizePolicyLabel(label string) string {
	label = strings.TrimPrefix(label, "@@")
	label = strings.TrimPrefix(label, "@")
	if i := strings.Index(label, "//"); i > 0 {
		label = label[i:]
	}
	if !strings.Contains(label, ":") {
		label += ":" + label[strings.LastIndex(label, "/")+1:]
	}
	return label
}
