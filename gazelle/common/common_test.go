package common

import (
	"reflect"
	"testing"
)

func TestNormalize(t *testing.T) {
	cases := []struct{ in, want string }{
		{"hello", "hello"},
		{"my-mod", "my_mod"},
		{"UPPER123", "UPPER123"},
	}
	for _, tc := range cases {
		if got, err := Normalize(tc.in); err != nil || got != tc.want {
			t.Errorf("Normalize(%q) = %q, %v; want %q", tc.in, got, err, tc.want)
		}
	}
	if _, err := Normalize("---"); err == nil {
		t.Errorf("Normalize(%q) succeeded, want error", "---")
	}
}

func TestCheckCollisions(t *testing.T) {
	if err := CheckCollisions([]Claimant{{Name: "a", Source: "a.js", Kind: "k"}}); err != nil {
		t.Fatalf("CheckCollisions = %v, want nil", err)
	}
	err := CheckCollisions([]Claimant{
		{Name: "a", Source: "a.js", Kind: "k"},
		{Name: "a", Source: "b.js", Kind: "k"},
	})
	if err == nil {
		t.Fatalf("CheckCollisions succeeded, want collision")
	}
}

func collectScan(src []byte) []string {
	var out []string
	Scan(src, func(spec string) {
		out = append(out, NormalizeJSSpec(spec))
	})
	return out
}

func collectEmbedded(src []byte) []string {
	var out []string
	ScanEmbedded(src, func(spec string) {
		out = append(out, NormalizeJSSpec(spec))
	})
	return out
}

func TestScanPlain(t *testing.T) {
	got := collectScan([]byte("import hello from \"./hello.js\";\n"))
	if !reflect.DeepEqual(got, []string{"hello"}) {
		t.Fatalf("Scan = %q, want [hello]", got)
	}
}

func TestScanEmbedded(t *testing.T) {
	src := []byte("<template><p>x</p></template><script>import y from \"./why.js\";</script>")
	got := collectEmbedded(ExtractScript(src))
	if !reflect.DeepEqual(got, []string{"why"}) {
		t.Fatalf("ScanEmbedded = %q, want [why]", got)
	}
}

func TestMaskStyles(t *testing.T) {
	src := []byte("import a.B;\n// import hidden.C;\n")
	if got := string(MaskJavaStyle(src)); got == string(src) {
		t.Fatalf("MaskJavaStyle left source unchanged")
	}
	if got := string(MaskCSharpStyle(src)); got == string(src) {
		t.Fatalf("MaskCSharpStyle left source unchanged")
	}
}

func TestNormalizeDotted(t *testing.T) {
	if got := NormalizeDotted("a.b.C", false, func(string) bool { return false }); got != "C" {
		t.Errorf("NormalizeDotted = %q, want C", got)
	}
}
