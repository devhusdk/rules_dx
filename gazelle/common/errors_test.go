package common

import (
	"reflect"
	"testing"
)

func TestErrorSinkSortsAndDedups(t *testing.T) {
	s := &ErrorSink{}
	s.Add("go", "", "go: b", "go: a")
	s.Add("cc", "pkg", "cc: z")
	s.Add("go", "", "go: a")
	s.Add("cc", "", "cc: y")
	got := s.Take()
	want := []GenerationError{
		{Language: "cc", Message: "cc: y"},
		{Language: "cc", Package: "pkg", Message: "cc: z"},
		{Language: "go", Message: "go: a"},
		{Language: "go", Message: "go: b"},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("Take() = %#v, want %#v", got, want)
	}
}

func TestErrorSinkTakeEmpties(t *testing.T) {
	s := &ErrorSink{}
	s.Add("go", "", "go: a")
	if got := s.Take(); len(got) != 1 {
		t.Fatalf("first Take() = %#v, want one entry", got)
	}
	if got := s.Take(); got != nil {
		t.Fatalf("second Take() = %#v, want nil", got)
	}
	s.Add("go", "", "go: a")
	s.Reset()
	if got := s.Take(); got != nil {
		t.Fatalf("Take() after Reset = %#v, want nil", got)
	}
}
