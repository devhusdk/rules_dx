package mdx

import "testing"

func TestTargetName(t *testing.T) {
	cases := []struct {
		name string
		want string
	}{
		{"demo.mdx", "demo"},
		{"pkg/my-mod.mdx", "my_mod"},
		{"Hello.mdx", "Hello"},
		{"plain", "plain"},
	}
	for _, tc := range cases {
		if got, err := TargetName(tc.name); err != nil || got != tc.want {
			t.Errorf("TargetName(%q) = %q, %v; want %q", tc.name, got, err, tc.want)
		}
	}
	if _, err := TargetName("---.mdx"); err == nil {
		t.Error("TargetName(---.mdx) succeeded, want error")
	}
}

func TestModuleName(t *testing.T) {
	cases := []struct {
		name string
		want string
	}{
		{"demo.mdx", "demo"},
		{"pkg/my-mod.mdx", "my-mod"},
		{"Hello.mdx", "Hello"},
		{"archive.tar.mdx", "archive.tar"},
		{"plain", "plain"},
	}
	for _, tc := range cases {
		if got := ModuleName(tc.name); got != tc.want {
			t.Errorf("ModuleName(%q) = %q, want %q", tc.name, got, tc.want)
		}
	}
}
