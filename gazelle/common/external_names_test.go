package common

import (
	"strings"
	"testing"
)

func TestNamedLockResolvesImports(t *testing.T) {
	cases := []struct {
		eco, lockText string
		imports       map[string]string
		unknown       string
	}{
		{
			eco: "hex",
			lockText: `{"ecosystem": "hex", "packages": [{"name": "jason", "provides": ["jason"]}, {"name": "phoenix_live_view", "provides": ["phoenix_live_view"]}], "schema": 1}`,
			imports: map[string]string{
				"Jason":                   "@hex//jason",
				"Jason.Encoder":           "@hex//jason",
				"Phoenix.LiveView.Router": "@hex//phoenix_live_view",
			},
			unknown: "Local",
		},
		{
			eco:      "nuget",
			lockText: "NUGET\n  remote: https://api.nuget.org/v3/index.json\n    Newtonsoft.Json (13.0.3)\n    Azure.Core (1.62)\n      Newtonsoft.Json (>= 13.0.3)\nGITHUB\n  remote: o/r\n    Missing.Namespace (abc)\n",
			imports: map[string]string{
				"Newtonsoft.Json":      "@paket.main//newtonsoft.json",
				"Newtonsoft.Json.Linq": "@paket.main//newtonsoft.json",
				"Azure.Core":           "@paket.main//azure.core",
			},
			unknown: "Missing.Namespace",
		},
	}
	for _, tc := range cases {
		t.Run(tc.eco, func(t *testing.T) {
			e := registryEntry(tc.eco)
			cfg, root := externalConfig(t)
			dotFile(t, root, e.Lock, tc.lockText)
			if errs := ConfigureExternal(cfg, "", nil); errs != nil {
				t.Fatal(errs)
			}
			for imp, want := range tc.imports {
				got, _ := ExternalLabels(cfg, tc.eco, imp)
				if labelStrings(got) != want {
					t.Errorf("%s = %q, want %q", imp, labelStrings(got), want)
				}
			}
			got, hint := ExternalLabels(cfg, tc.eco, tc.unknown)
			if got != nil || !strings.Contains(hint, e.Manifest) {
				t.Errorf("%s = %v, %q; want a hint naming %s", tc.unknown, got, hint, e.Manifest)
			}
		})
	}
}
