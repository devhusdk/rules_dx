package common

import "testing"

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
