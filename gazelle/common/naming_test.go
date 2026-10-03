package common

import "testing"

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
