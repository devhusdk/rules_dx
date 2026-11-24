package common

import (
	"path/filepath"
	"strings"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

const ownedConfigsKey = "dx_owned_configs"

type ownedConfigs map[string]string

func ConfigureOwnedConfigs(c *config.Config, rel string) {
	if rel == "" {
		c.Exts[ownedConfigsKey] = ownedConfigs(InitOwnedConfigs(c.RepoRoot))
	}
}

func OwnsRootConfig(c *config.Config, tool, file string) bool {
	owned, _ := c.Exts[ownedConfigsKey].(ownedConfigs)
	return owned[tool] == "//:"+file
}

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
