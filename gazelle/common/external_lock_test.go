package common

import (
	"fmt"
	"strings"
	"testing"
)

func TestLockResolvesEveryRegistryEcosystem(t *testing.T) {
	if len(lockRegistry) == 0 {
		t.Fatal("lock registry is empty")
	}
	for _, e := range lockRegistry {
		t.Run(e.Ecosystem, func(t *testing.T) {
			cfg, root := externalConfig(t)
			dotFile(t, root, e.Lock, fmt.Sprintf(`{"ecosystem": %q, "packages": [
				{"name": "alpha", "provides": ["alpha", "alpha/util"]},
				{"name": "alpha-extra", "provides": ["alpha/util/deep"]}]}`, e.Ecosystem))
			dotFile(t, root, "sub/"+e.Lock, fmt.Sprintf(`{"ecosystem": %q, "packages": [{"name": "beta", "provides": ["beta"]}]}`, e.Ecosystem))
			if errs := ConfigureExternal(cfg, "", nil); errs != nil {
				t.Fatal(errs)
			}
			for imp, want := range map[string]string{
				"alpha":           "@" + e.Hub + "//alpha",
				"alpha/util":      "@" + e.Hub + "//alpha",
				"alpha/util/deep": "@" + e.Hub + "//alpha-extra",
				"alpha/util/x":    "@" + e.Hub + "//alpha",
			} {
				got, _ := ExternalLabels(cfg, e.Ecosystem, imp)
				if labelStrings(got) != want {
					t.Errorf("%s = %q, want %q", imp, labelStrings(got), want)
				}
			}
			got, hint := ExternalLabels(cfg, e.Ecosystem, "unknown")
			if got != nil || !strings.Contains(hint, e.Manifest) || !strings.Contains(hint, e.Lock) {
				t.Errorf("unknown = %v, %q; want a hint naming %s", got, hint, e.Manifest)
			}
			if errs := ConfigureExternal(cfg, "sub", nil); errs != nil {
				t.Fatal(errs)
			}
			got, _ = ExternalLabels(cfg, e.Ecosystem, "beta")
			if want := "@" + e.Hub + "_sub//beta"; labelStrings(got) != want {
				t.Errorf("beta = %q, want %q", labelStrings(got), want)
			}
		})
	}
}
