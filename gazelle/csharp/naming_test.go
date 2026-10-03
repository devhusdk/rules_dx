package csharp

import "testing"

func TestIsTestSource(t *testing.T) {
	if !IsTestSource("DemoTest.cs") || !IsTestSource("pkg/DemoTest.cs") {
		t.Fatal("IsTestSource missed a *Test.cs file")
	}
	for _, name := range []string{"Demo.cs", "TestHelper.cs", "Contest.cs", "DemoTest.cs.bak"} {
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
	if got := ClassIdentity("pkg/Demo.cs"); got != "Demo" {
		t.Errorf("ClassIdentity = %q, want Demo", got)
	}
}
