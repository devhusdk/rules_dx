package zig

import (
	"strings"
	"testing"
)

func TestSourceClassification(t *testing.T) {
	if !IsTestSource("greet_test.zig") || IsTestSource("greet.zig") {
		t.Error("test source classification is wrong")
	}
	if !IsBuildScript("build.zig") || IsBuildScript("builder.zig") {
		t.Error("build script classification is wrong")
	}
}

func TestOrderRootFirst(t *testing.T) {
	cases := []struct {
		name    string
		sources []string
		want    string
	}{
		{"greet", []string{"a.zig", "greet.zig", "root.zig"}, "greet.zig,a.zig,root.zig"},
		{"greet", []string{"a.zig", "root.zig"}, "root.zig,a.zig"},
		{"greet", []string{"a.zig", "lib.zig"}, "lib.zig,a.zig"},
		{"greet", []string{"only.zig"}, "only.zig"},
	}
	for _, tc := range cases {
		got, err := OrderRootFirst(tc.name, tc.sources)
		if err != nil {
			t.Fatalf("OrderRootFirst(%s, %v): %v", tc.name, tc.sources, err)
		}
		if strings.Join(got, ",") != tc.want {
			t.Errorf("OrderRootFirst(%s, %v) = %v, want %s", tc.name, tc.sources, got, tc.want)
		}
	}
	_, err := OrderRootFirst("greet", []string{"a.zig", "b.zig"})
	if err == nil || !strings.Contains(err.Error(), "greet.zig, root.zig, lib.zig") {
		t.Errorf("err = %v, want the root candidates named", err)
	}
}
