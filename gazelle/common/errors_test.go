package common

import (
	"reflect"
	"strings"
	"testing"
)

func TestErrorSinkSortsAndDedups(t *testing.T) {
	s := &ErrorSink{}
	s.Add(GenerationError{Language: "go", Message: "go: b"}, GenerationError{Language: "go", Message: "go: a"})
	s.Add(GenerationError{Language: "cc", Package: InPackage("pkg"), Message: "cc: z"})
	s.Add(GenerationError{Language: "go", Message: "go: a"})
	s.Add(GenerationError{Language: "cc", Message: "cc: y"})
	got := s.Take()
	want := []GenerationError{
		{Language: "cc", Message: "cc: y"},
		{Language: "cc", Package: InPackage("pkg"), Message: "cc: z"},
		{Language: "go", Message: "go: a"},
		{Language: "go", Message: "go: b"},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("Take() = %#v, want %#v", got, want)
	}
}

func TestErrorSinkTakeEmpties(t *testing.T) {
	s := &ErrorSink{}
	s.Add(GenerationError{Language: "go", Message: "go: a"})
	if got := s.Take(); len(got) != 1 {
		t.Fatalf("first Take() = %#v, want one entry", got)
	}
	if got := s.Take(); got != nil {
		t.Fatalf("second Take() = %#v, want nil", got)
	}
	s.Add(GenerationError{Language: "go", Message: "go: a"})
	s.Reset()
	if got := s.Take(); got != nil {
		t.Fatalf("Take() after Reset = %#v, want nil", got)
	}
}

func TestSortErrorsDedups(t *testing.T) {
	in := []GenerationError{
		{Language: "go", Package: InPackage("b"), Message: "x"},
		{Language: "cc", Message: "y"},
		{Language: "go", Package: InPackage("b"), Message: "x"},
		{Language: "go", Package: InPackage("a"), Message: "z"},
	}
	want := []GenerationError{
		{Language: "cc", Message: "y"},
		{Language: "go", Package: InPackage("a"), Message: "z"},
		{Language: "go", Package: InPackage("b"), Message: "x"},
	}
	if got := SortErrors(in); !reflect.DeepEqual(got, want) {
		t.Fatalf("SortErrors = %+v, want %+v", got, want)
	}
	if got := SortErrors(nil); got != nil {
		t.Fatalf("SortErrors(nil) = %+v, want nil", got)
	}
}

func TestSortErrorsUnknownBeforeRoot(t *testing.T) {
	in := []GenerationError{
		{Language: "go", Package: InPackage("a"), Message: "m"},
		{Language: "go", Package: InPackage(""), Message: "m"},
		{Language: "go", Package: UnknownPackage, Message: "m"},
		{Language: "go", Package: InPackage(""), Message: "m"},
	}
	want := []GenerationError{
		{Language: "go", Package: UnknownPackage, Message: "m"},
		{Language: "go", Package: InPackage(""), Message: "m"},
		{Language: "go", Package: InPackage("a"), Message: "m"},
	}
	if got := SortErrors(in); !reflect.DeepEqual(got, want) {
		t.Fatalf("SortErrors = %+v, want %+v", got, want)
	}
}

func TestReportPrefixesKnownPackages(t *testing.T) {
	got := Report([]GenerationError{
		{Message: "run failed"},
		{Package: InPackage(""), Message: "root bad"},
		{Package: InPackage("a/b"), Message: "rust: malformed"},
		{Package: InPackage("a/b"), Message: "go: //a/b:x: bad"},
	})
	want := []string{"run failed", "//: root bad", "//a/b: rust: malformed", "go: //a/b:x: bad"}
	if strings.Join(got, "\n") != strings.Join(want, "\n") {
		t.Fatalf("Report = %q, want %q", got, want)
	}
}
