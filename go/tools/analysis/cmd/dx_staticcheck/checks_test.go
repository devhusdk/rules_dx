package main

import (
	"os"
	"path/filepath"
	"reflect"
	"slices"
	"strings"
	"testing"

	"golang.org/x/tools/go/analysis"
)

var names = []string{"S1000", "SA1000", "SA9003", "ST1000", "ST1003", "U1000"}

func sorted(set map[string]bool) []string {
	var out []string
	for name, on := range set {
		if on {
			out = append(out, name)
		}
	}
	slices.Sort(out)
	return out
}

func TestSelectChecks(t *testing.T) {
	cases := []struct {
		name  string
		rules []string
		want  []string
	}{
		{"all", []string{"all"}, names},
		{"star", []string{"*"}, names},
		{"negation", []string{"all", "-ST1000", "-U1000"}, []string{"S1000", "SA1000", "SA9003", "ST1003"}},
		{"category glob", []string{"S*"}, []string{"S1000"}},
		{"prefix glob", []string{"SA*", "ST1*"}, []string{"SA1000", "SA9003", "ST1000", "ST1003"}},
		{"number glob", []string{"SA9*"}, []string{"SA9003"}},
		{"name", []string{"st1003"}, []string{"ST1003"}},
		{"later wins", []string{"-ST1003", "ST1003"}, []string{"ST1003"}},
	}
	for _, c := range cases {
		if got := sorted(selectChecks(names, c.rules)); !reflect.DeepEqual(got, c.want) {
			t.Errorf("%s: got %v, want %v", c.name, got, c.want)
		}
	}
}

func writeConf(t *testing.T, body string) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "staticcheck.conf")
	if err := os.WriteFile(path, []byte(body), 0o644); err != nil {
		t.Fatal(err)
	}
	return path
}

func TestInherit(t *testing.T) {
	cfg, err := loadConfig(writeConf(t, `checks = ["inherit", "ST1003"]`))
	if err != nil {
		t.Fatal(err)
	}
	if !slices.Contains(cfg.Checks, "-ST1000") || cfg.Checks[len(cfg.Checks)-1] != "ST1003" {
		t.Errorf("checks = %v", cfg.Checks)
	}
	if !slices.Contains(names2(selectAnalyzers(cfg)), "ST1003") || slices.Contains(names2(selectAnalyzers(cfg)), "ST1000") {
		t.Error("ST1003 must be on and ST1000 must stay off")
	}
}

func TestInitialismsFromFile(t *testing.T) {
	cfg, err := loadConfig(writeConf(t, `initialisms = ["inherit", "HttpServer"]`))
	if err != nil {
		t.Fatal(err)
	}
	if !slices.Contains(cfg.Initialisms, "HttpServer") || !slices.Contains(cfg.Initialisms, "HTTP") {
		t.Errorf("initialisms = %v", cfg.Initialisms)
	}
	other, err := loadConfig(writeConf(t, `initialisms = ["XYZ"]`))
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(other.Initialisms, []string{"XYZ"}) {
		t.Errorf("initialisms = %v", other.Initialisms)
	}
}

func TestBadConfig(t *testing.T) {
	if _, err := loadConfig(writeConf(t, `checks = `)); err == nil {
		t.Error("want a parse error")
	}
	if _, err := loadConfig(filepath.Join(t.TempDir(), "missing.conf")); err == nil {
		t.Error("want an error for a missing file")
	}
}

func names2(list []*analysis.Analyzer) []string {
	var out []string
	for _, a := range list {
		out = append(out, a.Name)
	}
	return out
}

func TestDefaultMatchesStandardProfile(t *testing.T) {
	conf := filepath.Join(os.Getenv("TEST_SRCDIR"), os.Getenv("TEST_WORKSPACE"), os.Getenv("STANDARD_CONF"))
	data, err := os.ReadFile(conf)
	if err != nil {
		t.Fatal(err)
	}
	profile, err := loadConfig(conf)
	if err != nil {
		t.Fatal(err)
	}
	got := names2(selectAnalyzers(defaultConfig()))
	want := names2(selectAnalyzers(profile))
	if !reflect.DeepEqual(got, want) {
		t.Errorf("default checks differ from standard.conf:\n%v\n%v", got, want)
	}
	for _, off := range []string{"ST1000", "ST1003", "SA9003"} {
		if slices.Contains(got, off) || strings.Contains(string(data), "\""+off+"\"") {
			t.Errorf("%s should be off in both", off)
		}
	}
}
