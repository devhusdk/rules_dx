package golang

import "testing"

func TestIsTestSource(t *testing.T) {
	if !IsTestSource("demo_test.go") || !IsTestSource("pkg/demo_test.go") {
		t.Fatal("IsTestSource missed a *_test.go file")
	}
	for _, name := range []string{"demo.go", "test.go", "contest.go", "demo_test.go.bak"} {
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
