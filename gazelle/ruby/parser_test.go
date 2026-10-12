package ruby

import (
	"reflect"
	"testing"
)

func TestParseImportsSingleRequire(t *testing.T) {
	got := ParseImports([]byte("require \"acme/widget\"\nrequire \"json\"\n"))
	want := []string{"acme/widget", "json"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsSingleQuotes(t *testing.T) {
	got := ParseImports([]byte("require 'rspec'\n"))
	want := []string{"rspec"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsRequireRelativeInert(t *testing.T) {
	got := ParseImports([]byte("require_relative \"helper\"\nrequire \"rspec\"\n"))
	want := []string{"rspec"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsCommentsInert(t *testing.T) {
	got := ParseImports([]byte("# require \"acme/commented\"\nrequire \"json\"\n"))
	want := []string{"json"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsDedupedSorted(t *testing.T) {
	got := ParseImports([]byte("require \"zeta\"\nrequire \"alpha\"\nrequire \"zeta\"\n"))
	want := []string{"alpha", "zeta"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsBlockCommentsAndEmptyInert(t *testing.T) {
	got := ParseImports([]byte("=begin\nrequire \"acme/hidden\"\n=end\nrequire \"json\"\n"))
	want := []string{"json"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
	if got := ParseImports([]byte("require \" \"\nrequire \"json\"\n")); !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports blank require = %v, want %v", got, want)
	}
	if got := ParseImports([]byte("require \"./local\"\nrequire \"json\"\n")); !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports relative require = %v, want %v", got, want)
	}
}

func TestParsePackageAlwaysEmpty(t *testing.T) {
	if got, err := ParsePackage([]byte("module Demo; end\n")); err != nil || got != "" {
		t.Errorf("ParsePackage = %q, %v; want empty", got, err)
	}
}

func TestDefinesMain(t *testing.T) {
	if !DefinesMain([]byte("puts Hello.hello(\"world\") if __FILE__ == $0\n")) {
		t.Error("DefinesMain missed an executable guard")
	}
	if DefinesMain([]byte("# if __FILE__ == $0\nmodule Demo; end\n")) {
		t.Error("DefinesMain matched a commented guard")
	}
	if DefinesMain([]byte("module Demo; end\n")) {
		t.Error("DefinesMain matched a library source")
	}
}

func TestIsStdLib(t *testing.T) {
	for _, imp := range []string{"json", "yaml", "net/http", "fileutils"} {
		if !IsStdLib(imp) {
			t.Errorf("IsStdLib(%q) = false, want true", imp)
		}
	}
	for _, imp := range []string{"rspec", "acme/widget", "rake"} {
		if IsStdLib(imp) {
			t.Errorf("IsStdLib(%q) = true, want false", imp)
		}
	}
}
