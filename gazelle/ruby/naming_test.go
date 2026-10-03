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
