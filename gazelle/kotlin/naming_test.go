package kotlin

import "testing"

func TestIsTestSource(t *testing.T) {
	if !IsTestSource("DemoTest.kt") || !IsTestSource("pkg/DemoTest.kt") {
		t.Fatal("IsTestSource missed a *Test.kt file")
	}
	for _, name := range []string{"Demo.kt", "TestHelper.kt", "Contest.kt", "DemoTest.kt.bak"} {
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
	if got := ClassIdentity("pkg/Demo.kt"); got != "Demo" {
		t.Errorf("ClassIdentity = %q, want Demo", got)
	}
}
