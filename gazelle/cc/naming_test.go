package cc

import (
	"path/filepath"
	"reflect"
	"testing"
)

func TestIsTestSource(t *testing.T) {
	for _, name := range []string{"demo_test.cc", "pkg/demo_test.cc", "demo_test.h", "demo_test.cpp", "demo_test.cxx", "demo_test.c", "demo_test.hpp"} {
		if !IsTestSource(name) {
			t.Errorf("IsTestSource(%q) = false, want true", name)
		}
	}
	for _, name := range []string{"demo.cc", "test.cc", "contest.cc", "demo_test.cc.bak", "test_demo.cc", "demo.h", "demo_test.cc.bak"} {
		if IsTestSource(name) {
			t.Errorf("IsTestSource(%q) = true, want false", name)
		}
	}
}

func TestIsSourceIsHeader(t *testing.T) {
	for _, name := range []string{"a.c", "a.cc", "a.cpp", "a.cxx"} {
		if !IsSource(name) || IsHeader(name) {
			t.Errorf("source classification of %q wrong", name)
		}
	}
	for _, name := range []string{"a.h", "a.hh", "a.hpp", "a.hxx"} {
		if !IsHeader(name) || IsSource(name) {
			t.Errorf("header classification of %q wrong", name)
		}
	}
	for _, name := range []string{"a.cu", "a.cuh", "a.S", "notes.txt"} {
		if IsSource(name) || IsHeader(name) {
			t.Errorf("out-of-scope %q recognized, want unrecognized", name)
		}
	}
}

func TestHeaderIdentity(t *testing.T) {
	if got := HeaderIdentity("sub/./hello.h"); got != "sub/hello.h" {
		t.Errorf("HeaderIdentity = %q, want sub/hello.h", got)
	}
}

func TestHeaderBasename(t *testing.T) {
	if got := HeaderBasename(filepath.Join("ws", "pkg", "hello.h")); !reflect.DeepEqual(got, []string{"hello.h"}) {
		t.Errorf("HeaderBasename = %v, want [hello.h]", got)
	}
}

func TestIncludeKeys(t *testing.T) {
	for _, c := range []struct {
		pkg, name string
		want      []string
	}{
		{"a", "config.h", []string{"a/config.h", "config.h"}},
		{"a", "sub/x.h", []string{"a/sub/x.h", "sub/x.h"}},
		{"", "config.h", []string{"config.h"}},
		{"a", "../b/x.h", []string{"b/x.h"}},
		{"a", "../../x.h", nil},
		{"a", "./x.h", []string{"a/x.h", "x.h"}},
	} {
		if got := IncludeKeys(c.pkg, c.name); !reflect.DeepEqual(got, c.want) {
			t.Errorf("IncludeKeys(%q, %q) = %v, want %v", c.pkg, c.name, got, c.want)
		}
	}
}

func TestDirTargetName(t *testing.T) {
	got, err := DirTargetName("pkg/demo-pkg")
	if err != nil || got != "demo_pkg" {
		t.Errorf("DirTargetName = %q, %v; want demo_pkg", got, err)
	}
}

func TestCudaClassification(t *testing.T) {
	if !IsCudaSource("a.cu") || IsCudaSource("a.cuh") || IsCudaHeader("a.cu") || !IsCudaHeader("p/a.cuh") {
		t.Error("cuda classification wrong")
	}
	if IsCudaSource("a.cc") || IsCudaHeader("a.h") {
		t.Error("cc files classified as cuda")
	}
}
