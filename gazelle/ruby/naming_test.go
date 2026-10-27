package ruby

import "testing"

func TestIsTestSource(t *testing.T) {
	if !IsTestSource("demo_spec.rb") || !IsTestSource("pkg/demo_test.rb") {
		t.Fatal("IsTestSource missed a *_spec.rb or *_test.rb file")
	}
	for _, name := range []string{"demo.rb", "spec_helper.rb", "contest.rb", "demo_spec.rb.bak"} {
		if IsTestSource(name) {
			t.Errorf("IsTestSource(%q) = true, want false", name)
		}
	}
}

func TestDirTargetName(t *testing.T) {
	got, err := DirTargetName("pkg/demo-pkg")
	if err != nil || got != "demo_pkg" {
		t.Errorf("DirTargetName = %q, %v; want demo_pkg", got, err)
	}
}

func TestClassIdentity(t *testing.T) {
	if got := ClassIdentity("pkg/demo.rb"); got != "demo" {
		t.Errorf("ClassIdentity = %q, want demo", got)
	}
}

func TestLoadPath(t *testing.T) {
	cases := []struct{ pkg, src, want string }{
		{"lib/acme", "widget.rb", "acme/widget"},
		{"gems/x/lib/acme", "widget.rb", "acme/widget"},
		{"lib", "widget.rb", "widget"},
		{"app", "main.rb", "main"},
		{"", "a.rb", "a"},
		{"lib/x/lib", "y.rb", "y"},
		{"lib", "acme/widget.rb", "acme/widget"},
		{"app", "sub/main.rb", "sub/main"},
		{"", "lib/a/b.rb", "a/b"},
	}
	for _, c := range cases {
		if got := LoadPath(c.pkg, c.src); got != c.want {
			t.Errorf("LoadPath(%q, %q) = %q, want %q", c.pkg, c.src, got, c.want)
		}
	}
}
