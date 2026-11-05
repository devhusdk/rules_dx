package main

import (
	"encoding/json"
	"testing"
)

func TestFilterQuotesAndAnchorsImportPaths(t *testing.T) {
	got := filter([]string{"example.com/a.b", "x/y"})
	want := `^(example\.com/a\.b|x/y)$`
	if got != want {
		t.Fatalf("filter = %q, want %q", got, want)
	}
}

func TestArgsAddTestOnlyForTests(t *testing.T) {
	cfg := config{Packages: []string{"p"}, Workspace: []string{"p"}}
	if got := args(cfg); len(got) != 3 || got[1] != "-filter=^(p)$" || got[2] != "p" {
		t.Fatalf("args = %q", got)
	}
	cfg.Test = true
	if got := args(cfg); len(got) != 4 || got[1] != "-test" {
		t.Fatalf("args = %q", got)
	}
}

func TestRewriteMapsWorkspaceFilesAndDropsTheRest(t *testing.T) {
	in := `[{"Name":"lib","Path":"ex/lib","Funcs":[
		{"Name":"Uncalled","Position":{"File":"src/ex/lib/lib.go","Line":3,"Col":6}},
		{"Name":"Gen","Position":{"File":"/g/src/ex/lib/gen.go","Line":1,"Col":1}}]},
		{"Name":"dep","Path":"ex/dep","Funcs":[
		{"Name":"X","Position":{"File":"src/ex/dep/dep.go","Line":1,"Col":1}}]}]`
	files := map[string]string{
		"ex/lib/lib.go": "pkg/lib/lib.go",
		"ex/lib/gen.go": "pkg/lib/gen.go",
	}
	out, err := rewrite([]byte(in), "/g", files)
	if err != nil {
		t.Fatal(err)
	}
	var got []pkg
	if err := json.Unmarshal(out, &got); err != nil {
		t.Fatal(err)
	}
	if len(got) != 1 || len(got[0].Funcs) != 2 {
		t.Fatalf("got %+v", got)
	}
	if got[0].Funcs[0].Position.File != "pkg/lib/lib.go" || got[0].Funcs[1].Position.File != "pkg/lib/gen.go" {
		t.Fatalf("got %+v", got)
	}
}

func TestRewriteOfNothingIsAnEmptyList(t *testing.T) {
	out, err := rewrite(nil, "/g", nil)
	if err != nil || string(out) != "[]" {
		t.Fatalf("out = %q, err = %v", out, err)
	}
}

func TestEnvironmentIsHermeticGOPATHMode(t *testing.T) {
	env := environment(config{GOOS: "linux"}, "/g", "/t")
	for _, want := range []string{"GOPATH=/g", "GO111MODULE=off", "CGO_ENABLED=0", "GOWORK=off", "GOTOOLCHAIN=local", "GOFLAGS=", "GOOS=linux"} {
		found := false
		for _, entry := range env {
			found = found || entry == want
		}
		if !found {
			t.Errorf("missing %q in %q", want, env)
		}
	}
}
