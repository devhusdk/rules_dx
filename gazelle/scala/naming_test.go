package scala

import "testing"

func TestIsTestSource(t *testing.T) {
	if !IsTestSource("DemoTest.scala") || !IsTestSource("pkg/DemoTest.scala") {
		t.Fatal("IsTestSource missed a *Test.scala file")
	}
	for _, name := range []string{"Demo.scala", "TestHelper.scala", "Contest.scala", "DemoTest.scala.bak"} {
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
	if got := ClassIdentity("pkg/Demo.scala"); got != "Demo" {
		t.Errorf("ClassIdentity = %q, want Demo", got)
	}
}
