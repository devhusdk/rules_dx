package swift

import (
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

func TestImportsUsesModuleName(t *testing.T) {
	l := &swiftLang{}
	plain := rule.NewRule(LibraryKind, "greeter")
	if got := l.Imports(nil, plain, nil); len(got) != 1 || got[0].Imp != "greeter" {
		t.Errorf("Imports = %v", got)
	}
	named := rule.NewRule(LibraryKind, "greeter")
	named.SetAttr("module_name", "Greeter")
	if got := l.Imports(nil, named, nil); len(got) != 1 || got[0].Imp != "Greeter" {
		t.Errorf("Imports = %v", got)
	}
	if got := l.Imports(nil, rule.NewRule(TestKind, "t"), nil); got != nil {
		t.Errorf("test rule indexed: %v", got)
	}
}

func TestIsTestSource(t *testing.T) {
	if !IsTestSource("GreeterTests.swift") || IsTestSource("Greeter.swift") {
		t.Error("IsTestSource classifies wrongly")
	}
}
