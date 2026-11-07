package common

import (
	"path/filepath"
	"strings"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

const ownedConfigsKey = "dx_owned_configs"

type ownedConfigs map[string]string

// ConfigureOwnedConfigs reads the config bindings dx generate wrote to
// .dx/BUILD.bazel once, at the repository root. Packages below inherit them.
func ConfigureOwnedConfigs(c *config.Config, rel string) {
	if rel == "" {
		c.Exts[ownedConfigsKey] = ownedConfigs(InitOwnedConfigs(c.RepoRoot))
	}
}

// OwnsRootConfig reports whether .dx/BUILD.bazel binds tool to the root file.
func OwnsRootConfig(c *config.Config, tool, file string) bool {
	owned, _ := c.Exts[ownedConfigsKey].(ownedConfigs)
	return owned[tool] == "//:"+file
}

// InitOwnedConfigs maps each tool to the src of its <tool>_config rule in
// .dx/BUILD.bazel, and returns nil when the file does not parse.
func InitOwnedConfigs(repoRoot string) map[string]string {
	f, err := rule.LoadFile(filepath.Join(repoRoot, ".dx", "BUILD.bazel"), ".dx")
	if err != nil {
		return nil
	}
	owned := map[string]string{}
	for _, r := range f.Rules {
		tool, ok := strings.CutSuffix(r.Kind(), "_config")
		if !ok || tool == "" {
			continue
		}
		if src := r.AttrString("src"); src != "" {
			owned[tool] = src
		}
	}
	return owned
}
