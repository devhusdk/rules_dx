package golang

import (
	"reflect"
	"testing"
)

func TestParseImportsLocalKeptQualified(t *testing.T) {
	got, err := ParseImports([]byte("package demo\n\nimport (\n\t\"fmt\"\n\t\"rules_dx/go/tests/fixtures/hello\"\n)\n"))
	if err != nil {
		t.Fatal(err)
	}
	want := []string{"fmt", "rules_dx/go/tests/fixtures/hello"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsThirdPartyKeptFull(t *testing.T) {
	got, err := ParseImports([]byte("package demo\n\nimport _ \"github.com/x/y\"\n"))
	if err != nil {
		t.Fatal(err)
	}
	want := []string{"github.com/x/y"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsCommentsAndStringsInert(t *testing.T) {
	got, err := ParseImports([]byte("package demo\n\n// import \"rules_dx/fake/comment\"\n/* import \"rules_dx/fake/block\" */\nimport \"fmt\"\n\nvar s = \"import rules_dx/fake/string\"\n"))
	if err != nil {
		t.Fatal(err)
	}
	want := []string{"fmt"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsDedupedSorted(t *testing.T) {
	got, err := ParseImports([]byte("package demo\n\nimport (\n\t\"rules_dx/zeta\"\n\t\"rules_dx/alpha\"\n\t\"rules_dx/zeta\"\n)\n"))
	if err != nil {
		t.Fatal(err)
	}
	want := []string{"rules_dx/alpha", "rules_dx/zeta"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsEmptyPathInert(t *testing.T) {
	got, err := ParseImports([]byte("package demo\n\nimport \"\"\n"))
	if err != nil {
		t.Fatal(err)
	}
	if len(got) != 0 {
		t.Errorf("ParseImports = %v, want empty", got)
	}
}

func TestParseImportsUnparseableFails(t *testing.T) {
	if _, err := ParseImports([]byte("package demo\n\nimport (\n")); err == nil {
		t.Fatal("ParseImports succeeded on an unparseable file")
	}
}

func TestParsePackage(t *testing.T) {
	got, err := ParsePackage([]byte("package demo\n"))
	if err != nil || got != "demo" {
		t.Errorf("ParsePackage = %q, %v; want demo, nil", got, err)
	}
	if _, err := ParsePackage([]byte("package\n")); err == nil {
		t.Fatal("ParsePackage succeeded on an unparseable file")
	}
}

func TestIsStdLibExact(t *testing.T) {
	for _, name := range []string{"fmt", "net/http", "encoding/json", "sync/atomic", "testing"} {
		if !IsStdLib(name) {
			t.Errorf("IsStdLib(%q) = false, want true", name)
		}
	}
	for _, name := range []string{"", "hello", "rules_dx/go/tests/fixtures/hello", "github.com/x/y", "fmtx", "net/httpx"} {
		if IsStdLib(name) {
			t.Errorf("IsStdLib(%q) = true, want false", name)
		}
	}
}

func TestIsCgoImportExact(t *testing.T) {
	if !IsCgoImport("C") {
		t.Error(`IsCgoImport("C") = false, want true`)
	}
	for _, name := range []string{"", "c", "C/", "C/x", "fmt", "runtime/cgo"} {
		if IsCgoImport(name) {
			t.Errorf("IsCgoImport(%q) = true, want false", name)
		}
	}
	if IsStdLib("C") {
		t.Error(`IsStdLib("C") = true, want false (cgo is never stdlib)`)
	}
}

func TestParseImportsCgoSurfaces(t *testing.T) {
	got, err := ParseImports([]byte("package demo\n\nimport \"C\"\n"))
	if err != nil {
		t.Fatal(err)
	}
	if len(got) != 1 || got[0] != "C" {
		t.Fatalf("ParseImports cgo = %v, want [C] for fail-closed detection", got)
	}
}

func TestBuildExcluded(t *testing.T) {
	cases := []struct {
		name    string
		src     string
		want    bool
		wantErr bool
	}{
		{"no constraint", "package demo\n", false, false},
		{"ignore", "//go:build ignore\n\npackage main\n", true, false},
		{"tools", "//go:build tools\n\npackage tools\n", true, false},
		{"plus build ignore", "// +build ignore\n\npackage main\n", true, false},
		{"go build overrides plus build", "//go:build linux\n// +build ignore\n\npackage demo\n", false, false},
		{"linux", "//go:build linux\n\npackage demo\n", false, false},
		{"not windows and amd64", "//go:build !windows && amd64\n\npackage demo\n", false, false},
		{"linux and tools", "//go:build linux && tools\n\npackage tools\n", true, false},
		{"not tools", "//go:build !tools\n\npackage demo\n", false, false},
		{"linux and windows", "//go:build linux && windows\n\npackage demo\n", true, false},
		{"android and linux", "//go:build android && linux\n\npackage demo\n", false, false},
		{"amd64 and arm64", "//go:build amd64 && arm64\n\npackage demo\n", true, false},
		{"after package clause", "package demo\n\n//go:build ignore\n", false, false},
		{"custom tag", "//go:build integration\n\npackage demo\n", false, false},
		{"custom tag and linux", "//go:build integration && linux\n\npackage demo\n", false, false},
		{"not custom tag", "//go:build !integration\n\npackage demo\n", false, false},
		{"ignore or custom tag", "//go:build ignore || integration\n\npackage demo\n", false, false},
		{"plus build custom tag", "// +build integration\n\npackage demo\n", false, false},
		{"tools and custom tag", "//go:build tools && integration\n\npackage tools\n", true, false},
		{"two oses and custom tag", "//go:build linux && windows && integration\n\npackage demo\n", true, false},
		{"malformed", "//go:build &&\n\npackage demo\n", false, true},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got, err := BuildExcluded([]byte(tc.src))
			if (err != nil) != tc.wantErr {
				t.Fatalf("BuildExcluded error = %v, wantErr %v", err, tc.wantErr)
			}
			if got != tc.want {
				t.Errorf("BuildExcluded = %v, want %v", got, tc.want)
			}
		})
	}
}
