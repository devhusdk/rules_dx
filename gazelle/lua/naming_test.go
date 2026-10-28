package lua

import "testing"

// The vectors mirror lua/rules/wrapper_tests.bzl; keep the two in sync.
func TestModuleName(t *testing.T) {
	cases := []struct {
		name                     string
		path, pkg, strip, prefix string
		want                     string
		ok                       bool
	}{
		{"a/b.lua is module a.b", "a/b.lua", "", "", "", "a.b", true},
		{"a/init.lua is module a", "a/init.lua", "", "", "", "a", true},
		{"a root init.lua is module init", "init.lua", "", "", "", "init", true},
		{"a non-Lua file has no module", "a/b.txt", "", "", "", "", false},
		{"a dotted stem has no module", "a/b.c.lua", "", "", "", "", false},
		{"relative strip starts at the package", "pkg/src/x/y.lua", "pkg", "src", "", "x.y", true},
		{"absolute strip starts at the root", "pkg/src/x/init.lua", "pkg", "/pkg/src/", "", "x", true},
		{"outside strip has no module", "pkg/other/y.lua", "pkg", "src", "", "", false},
		{"import_prefix is added after stripping", "pkg/src/y.lua", "pkg", "src", "vendor.lib", "vendor.lib.y", true},
		{"root init under import_prefix is the prefix", "pkg/init.lua", "pkg", "/pkg", "inspect", "inspect", true},
	}
	for _, tc := range cases {
		got, ok := moduleNameOf(tc.path, tc.pkg, tc.strip, tc.prefix)
		if got != tc.want || ok != tc.ok {
			t.Errorf("%s: got (%q, %v), want (%q, %v)", tc.name, got, ok, tc.want, tc.ok)
		}
	}
}

func TestModuleNameJoinsPackage(t *testing.T) {
	if got, ok := ModuleName("pkg", "x/y.lua", "", ""); got != "pkg.x.y" || !ok {
		t.Errorf("got (%q, %v)", got, ok)
	}
}

func TestTargetNames(t *testing.T) {
	if got, err := TestTargetName("sub/catalog_test.lua"); got != "catalog_test" || err != nil {
		t.Errorf("TestTargetName = (%q, %v)", got, err)
	}
}
