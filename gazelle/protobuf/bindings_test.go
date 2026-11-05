package protobuf

import (
	"os"
	"path/filepath"
	"slices"
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
)

func TestGoImportPath(t *testing.T) {
	cases := map[string]string{
		`option go_package = "example.com/a;apb";`: "example.com/a",
		`option go_package = 'example.com/b';`:     "example.com/b",
		`// option go_package = "x";`:             "",
		`syntax = "proto3";`:                      "",
	}
	for in, want := range cases {
		if got := GoImportPath([]byte(in)); got != want {
			t.Errorf("GoImportPath(%q) = %q, want %q", in, got, want)
		}
	}
}

func TestJavaPackage(t *testing.T) {
	cases := map[string]string{
		"package a.b;\noption java_package = \"com.x\";": "com.x",
		"// package no;\npackage a.b;":                 "a.b",
		`syntax = "proto3";`:                           "",
	}
	for in, want := range cases {
		if got := JavaPackage([]byte(in)); got != want {
			t.Errorf("JavaPackage(%q) = %q, want %q", in, got, want)
		}
	}
}

func TestBindingImports(t *testing.T) {
	dir := t.TempDir()
	pkg := filepath.Join(dir, "api")
	if err := os.MkdirAll(pkg, 0o755); err != nil {
		t.Fatal(err)
	}
	src := "package api;\noption java_package = \"com.x\";\n"
	if err := os.WriteFile(filepath.Join(pkg, "user.proto"), []byte(src), 0o644); err != nil {
		t.Fatal(err)
	}
	c := &config.Config{RepoRoot: dir}
	f := rule.EmptyFile("api/BUILD.bazel", "api")
	lib := rule.NewRule(LibraryKind, "api_proto")
	lib.SetAttr("srcs", []string{"user.proto"})
	lib.Insert(f)

	binding := func(kind, attr string) *rule.Rule {
		r := rule.NewRule(kind, "api_x_proto")
		r.SetAttr(attr, []string{":api_proto"})
		if kind == goKind {
			r.SetAttr("proto", ":api_proto")
			r.SetAttr("importpath", "example.com/gen/api")
		}
		return r
	}
	cases := map[string][]string{
		goKind:   {"go:example.com/gen/api", "go_proto://api:api_proto"},
		pyKind:   {"python:api.user_pb2"},
		javaKind: {"java:com.x"},
		ccKind:   {"cc:api/user.pb.h"},
	}
	for kind, want := range cases {
		var got []string
		for _, spec := range bindingImports(c, binding(kind, "deps"), f) {
			got = append(got, spec.Imp)
		}
		if !slices.Equal(got, want) {
			t.Errorf("%s imports = %v, want %v", kind, got, want)
		}
	}
}

func TestCSharpImportsIndexProtoLibrary(t *testing.T) {
	c := &config.Config{RepoRoot: t.TempDir(), RepoName: "repo"}
	f := rule.EmptyFile("api/BUILD.bazel", "api")
	lib := rule.NewRule(LibraryKind, "api_proto")
	lib.SetAttr("srcs", []string{"user.proto"})
	lib.Insert(f)
	cs := rule.NewRule(csKind, "api_proto_cs")
	cs.SetAttr("proto", ":api_proto")
	var got []string
	for _, spec := range csImports(c, cs, f) {
		got = append(got, spec.Imp)
	}
	want := []string{"cs_proto:@repo//api:api_proto"}
	if !slices.Equal(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}
