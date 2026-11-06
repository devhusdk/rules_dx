package common

import (
	"fmt"
	"strings"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

// PolicyConfigDirective binds a tool to the config target the root
// workspace_policy lists in configs:
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
