package elixir

import (
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

func TestParseMix(t *testing.T) {
	cases := []struct {
		name string
		src  string
		want Mix
	}{
		{"literal", `defmodule App.MixProject do
  use Mix.Project
  def project, do: [app: :app, version: "0.2.0", deps: deps()]
  def application, do: [mod: {App.Application, []}, extra_applications: [:logger]]
  defp deps, do: [{:jason, "~> 1.4", app: false}]
end
`, Mix{App: "app", Version: "0.2.0", Mod: "App.Application"}},
		{"attribute", `defmodule X.MixProject do
  @version "1.2.3"
  def project, do: [app: :x, version: @version]
end
`, Mix{App: "x", Version: "1.2.3"}},
		{"missing version", `def project, do: [app: :y]`, Mix{App: "y"}},
		{"umbrella", `def project, do: [apps_path: "apps", version: "0.1.0"]`, Mix{Version: "0.1.0", Umbrella: true}},
		{"commented app", "# app: :old,\n@doc \"app: :doc\"\ndef project, do: [app: :new]", Mix{App: "new"}},
		{"interpolated version", `def project, do: [app: :z, version: "#{@v}-dev"]`, Mix{App: "z"}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if got := ParseMix([]byte(tc.src)); got != tc.want {
				t.Fatalf("ParseMix = %+v, want %+v", got, tc.want)
			}
		})
	}
}

func TestOwns(t *testing.T) {
	mk := func(kind, name string, srcs ...string) *rule.Rule {
		r := rule.NewRule(kind, name)
		if len(srcs) > 0 {
			r.SetAttr("srcs", srcs)
		}
		return r
	}
	cases := []struct {
		r    *rule.Rule
		want bool
	}{
		{mk(AppKind, "app", "lib/app.ex", "lib/app/x.ex"), true},
		{mk(AppKind, "app"), false},
		{mk(AppKind, "", "lib/app.ex"), false},
		{mk(AppKind, "app", "lib/app.ex", ":gen"), false},
		{mk(AppKind, "app", "src/app.ex"), false},
		{mk(TestKind, "t", "test/a_test.exs", "test/test_helper.exs"), true},
		{mk(TestKind, "t", "test/support/case.ex"), false},
		{mk(TestKind, "t", "other_test.exs"), false},
		{mk("elixir_library", "x", "lib/x.ex"), false},
	}
	for _, tc := range cases {
		if got := Owns(tc.r); got != tc.want {
			t.Errorf("Owns(%s %q %v) = %v, want %v", tc.r.Kind(), tc.r.Name(), tc.r.AttrStrings("srcs"), got, tc.want)
		}
	}
}
