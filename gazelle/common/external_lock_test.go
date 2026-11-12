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

func TestLockLookupTrimsNamespaceAndKeepsSharedProvides(t *testing.T) {
	cfg, root := externalConfig(t)
	dotFile(t, root, "composer.lock.json", `{"ecosystem": "composer", "packages": [
		{"name": "psr/http-factory", "provides": ["Psr\\Http\\Message"]},
		{"name": "psr/http-message", "provides": ["Psr\\Http\\Message"]}]}`)
	if errs := ConfigureExternal(cfg, "", nil); errs != nil {
		t.Fatal(errs)
	}
	got, _ := ExternalLabels(cfg, "composer", `Psr\Http\Message\RequestInterface`)
	if want := "@composer//psr/http-factory:psr/http-factory,@composer//psr/http-message:psr/http-message"; labelStrings(got) != want {
		t.Errorf("RequestInterface = %q, want %q", labelStrings(got), want)
	}
}

func TestLockLookupDubModules(t *testing.T) {
	cfg, root := externalConfig(t)
	dotFile(t, root, "dub.lock.json", `{"ecosystem": "dub", "packages": [{"name": "vibe-d", "provides": ["vibe_d.core"]}, {"name": "silly", "provides": ["silly"]}, {"name": "mixed", "provides": ["mixed.sub"]}]}`)
	if errs := ConfigureExternal(cfg, "", nil); errs != nil {
		t.Fatal(errs)
	}
	for module, want := range map[string]string{
		"silly":                "@dub//silly",
		"vibe_d.core.log":      "@dub//vibe-d",
		"mixed.sub.thing":      "@dub//mixed",
		"std.stdio":            "",
		"unknown_package.core": "",
	} {
		got, _ := ExternalLabels(cfg, "dub", module)
		if labelStrings(got) != want {
			t.Errorf("%s = %q, want %q", module, labelStrings(got), want)
		}
	}
	if _, hint := ExternalLabels(cfg, "dub", "other.x"); !strings.Contains(hint, "no package in dub.lock.json provides other.x") {
		t.Errorf("hint = %q", hint)
	}
}
