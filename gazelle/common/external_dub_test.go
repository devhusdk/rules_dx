package common

import (
	"strings"
	"testing"
)

func TestDubExternalLookup(t *testing.T) {
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
