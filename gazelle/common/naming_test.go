package common

import (
	"testing"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
)

func TestStem(t *testing.T) {
	cases := []struct {
		name string
		exts []string
		in   string
		want string
	}{
		{"first ext wins", []string{".ts", ".tsx"}, "pkg/my-mod.tsx", "my-mod"},
		{"a later ext still strips", []string{".js", ".jsx", ".mjs"}, "entry.mjs", "entry"},
		{"the base name is dropped", []string{".astro"}, "a/b/demo.astro", "demo"},
		{"a foreign ext stays", []string{".ts"}, "demo.js", "demo.js"},
		{"no ext stays", []string{".ts"}, "plain", "plain"},
		{"only the last dot is not split", []string{".astro"}, "archive.tar.astro", "archive.tar"},
		{"no exts strips nothing", nil, "demo.ts", "demo.ts"},
	}
	for _, tc := range cases {
		if got := Stem(tc.in, tc.exts); got != tc.want {
			t.Errorf("Stem(%q, %v) = %q, want %q [%s]", tc.in, tc.exts, got, tc.want, tc.name)
		}
	}
}

func TestIsEntryFile(t *testing.T) {
	cases := []struct {
		name string
		exts []string
		in   string
		want bool
	}{
		{"first ext", []string{".ts", ".tsx"}, "main.ts", true},
		{"later ext", []string{".ts", ".tsx"}, "pkg/main.tsx", true},
		{"a declaration is not an entry", []string{".ts", ".tsx"}, "main.d.ts", false},
		{"the whole base name must be main", []string{".ts"}, "my.main.ts", false},
		{"a trailing suffix is not an ext", []string{".ts"}, "main.ts.bak", false},
		{"another name", []string{".ts", ".tsx"}, "index.ts", false},
		{"a foreign ext", []string{".ts"}, "main.js", false},
		{"no ext", []string{".ts"}, "main", false},
		{"a directory that ends in main.ts", []string{".ts"}, "src/main.ts", true},
	}
	for _, tc := range cases {
		if got := IsEntryFile(tc.in, tc.exts); got != tc.want {
			t.Errorf("IsEntryFile(%q, %v) = %v, want %v [%s]", tc.in, tc.exts, got, tc.want, tc.name)
		}
	}
}

func TestBinaryName(t *testing.T) {
	for in, want := range map[string]string{"main": "main_bin", "a_b": "a_b_bin", "": "_bin"} {
		if got := BinaryName(in); got != want {
			t.Errorf("BinaryName(%q) = %q, want %q", in, got, want)
		}
	}
}

func TestUnitTestName(t *testing.T) {
	for in, want := range map[string]string{"parser": "parser_test", "a_test": "a_test_test", "": "_test"} {
		if got := UnitTestName(in); got != want {
			t.Errorf("UnitTestName(%q) = %q, want %q", in, got, want)
		}
	}
}

func TestSnakeName(t *testing.T) {
	exts := []string{".java"}
	for _, tc := range []struct{ in, want string }{
		{"GreetTest.java", "greet_test"},
		{"HTTPClientTest.java", "http_client_test"},
		{"Foo2Test.java", "foo2_test"},
		{"greet_spec", "greet_spec"},
		{"pkg/InvoiceReaderTest.java", "invoice_reader_test"},
		{"ABTest.java", "ab_test"},
		{"Foo_BarTest.java", "foo_bar_test"},
	} {
		got, err := SnakeName(tc.in, exts)
		if err != nil || got != tc.want {
			t.Errorf("SnakeName(%q) = %q, %v; want %q", tc.in, got, err, tc.want)
		}
	}
	if got, err := SnakeName("---.java", exts); err == nil {
		t.Errorf("SnakeName(---.java) = %q, want an error", got)
	}
}

func TestClassIdentity(t *testing.T) {
	cases := []struct {
		exts []string
		in   string
		want string
	}{
		{[]string{".java"}, "pkg/Demo.java", "Demo"},
		{[]string{".java"}, "pkg/inner.Outer.java", "Outer"},
		{[]string{".java"}, "pkg/inner.Outer", "Outer"},
		{[]string{".cs"}, "pkg/Demo.cs", "Demo"},
		{[]string{".fs"}, "pkg/Demo.fs", "Demo"},
		{[]string{".kt"}, "pkg/Demo.kt", "Demo"},
		{[]string{".scala"}, "pkg/Demo.scala", "Demo"},
		{[]string{".rb"}, "pkg/demo.rb", "demo"},
	}
	for _, tc := range cases {
		if got := ClassIdentity(tc.in, tc.exts); got != tc.want {
			t.Errorf("ClassIdentity(%q, %v) = %q, want %q", tc.in, tc.exts, got, tc.want)
		}
	}
}

func TestIsTestSource(t *testing.T) {
	cases := []struct {
		name     string
		exts     []string
		suffixes []string
		in       string
		want     bool
	}{
		{"java suffix", []string{".java"}, []string{"Test"}, "pkg/DemoTest.java", true},
		{"java plain", []string{".java"}, []string{"Test"}, "Demo.java", false},
		{"java prefix only", []string{".java"}, []string{"Test"}, "TestHelper.java", false},
		{"java substring only", []string{".java"}, []string{"Test"}, "Contest.java", false},
		{"java suffixed backup", []string{".java"}, []string{"Test"}, "DemoTest.java.bak", false},
		{"ruby first suffix", []string{".rb"}, []string{"_spec", "_test"}, "demo_spec.rb", true},
		{"ruby second suffix", []string{".rb"}, []string{"_spec", "_test"}, "pkg/demo_test.rb", true},
		{"ruby plain", []string{".rb"}, []string{"_spec", "_test"}, "demo.rb", false},
		{"ruby prefix only", []string{".rb"}, []string{"_spec", "_test"}, "spec_helper.rb", false},
		{"ruby substring only", []string{".rb"}, []string{"_spec", "_test"}, "contest.rb", false},
		{"ruby suffixed backup", []string{".rb"}, []string{"_spec", "_test"}, "demo_spec.rb.bak", false},
		{"only the listed suffixes count", []string{".rb"}, []string{"_spec"}, "demo_test.rb", false},
		{"no suffixes never matches", []string{".java"}, nil, "DemoTest.java", false},
		{"an unstripped extension still counts", []string{".java"}, []string{"Test"}, "DemoTest", true},
		{"a foreign extension hides the suffix", []string{".java"}, []string{"Test"}, "DemoTest.kt", false},
	}
	for _, tc := range cases {
		if got := IsTestSource(tc.in, tc.exts, tc.suffixes); got != tc.want {
			t.Errorf("IsTestSource(%q, %v, %v) = %v, want %v [%s]", tc.in, tc.exts, tc.suffixes, got, tc.want, tc.name)
		}
	}
}

func TestRootTargetName(t *testing.T) {
	cases := []struct {
		hints []string
		want  string
	}{
		{[]string{"first", "second"}, "first"},
		{[]string{"---", "my-repo"}, "my_repo"},
		{[]string{"", "---"}, "lib"},
		{nil, "lib"},
	}
	for _, tc := range cases {
		if got := RootTargetName(tc.hints...); got != tc.want {
			t.Errorf("RootTargetName(%q) = %q, want %q", tc.hints, got, tc.want)
		}
	}
	if got, err := PackageTargetName(language.GenerateArgs{Rel: ""}); err != nil || got != "lib" {
		t.Errorf("PackageTargetName(root, nil config) = %q, %v, want lib", got, err)
	}
	if got, err := PackageTargetName(language.GenerateArgs{Rel: "", Config: &config.Config{RepoName: "demo-repo"}}, "---"); err != nil || got != "demo_repo" {
		t.Errorf("PackageTargetName(root, demo-repo) = %q, %v, want demo_repo", got, err)
	}
	if got, err := PackageTargetName(language.GenerateArgs{Rel: "pkg/a-b", Config: &config.Config{RepoName: "demo"}}, "hint"); err != nil || got != "a_b" {
		t.Errorf("PackageTargetName(pkg/a-b) = %q, %v, want a_b", got, err)
	}
	if _, err := DirTargetName("pkg/---"); err == nil {
		t.Error("DirTargetName(pkg/---) succeeded, want an empty-name error")
	}
	if _, err := PackageTargetName(language.GenerateArgs{Rel: "pkg/---"}, "hint"); err == nil {
		t.Error("PackageTargetName(pkg/---) succeeded, want an empty-name error")
	}
}
