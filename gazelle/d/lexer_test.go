package d

import (
	"strings"
	"testing"
)

func TestLexLiterals(t *testing.T) {
	// Each source ends in a marker identifier that must survive the literal.
	tests := []struct{ name, src string }{
		{"line comment", "// \"\nend"},
		{"block comment", "/* \" */ end"},
		{"nested comment", "/+ /+ \" +/ \" +/ end"},
		{"string", `"a\"b" end`},
		{"string postfix", `"a"c "b"w "c"d end`},
		{"wysiwyg", `r"a\" end`},
		{"backtick", "`a\\` end"},
		{"hex string", `x"0A 0B"c end`},
		{"delimited paren", `q"(a (b) ")"c end`},
		{"delimited bracket", `q"[a [b] "]" end`},
		{"delimited brace", `q"{a {b} "}" end`},
		{"delimited angle", `q"<a <b> ">" end`},
		{"delimited char", `q"/a"b/" end`},
		{"heredoc", "q\"EOS\nline \"\nEOS\"c end"},
		{"token string", "q{ int \"}\" /* } */ { x; } }c end"},
		{"interpolated", `i"a $(f("x)")) \" b" end`},
		{"interpolated backtick", "i`a $(1) b` end"},
		{"interpolated token string", "iq{ a } end"},
		{"char", `'a' '\'' '\\' 'ሴ' '\&amp;' 'x'd end`},
		{"numbers", "1 1.5 1e+5 0x1p-3 0xe+1 1_000 .5 1L 1u end"},
		{"shebang", "#!/usr/bin/rdmd\nend"},
		{"line directive", "#line 5 \"x.d\"\nend"},
		{"eof marker", "end __EOF__ \""},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			toks, err := lex(tc.src)
			if err != nil {
				t.Fatal(err)
			}
			if len(toks) == 0 || !toks[len(toks)-1].is("end") {
				t.Fatalf("last token of %v is not end", toks)
			}
			for _, tok := range toks {
				if tok.is("{") || tok.is("}") {
					t.Fatalf("brace leaked out of a literal: %v", toks)
				}
			}
		})
	}
}

func TestLexNumberRange(t *testing.T) {
	toks, err := lex("a[1..2]")
	if err != nil {
		t.Fatal(err)
	}
	var texts []string
	for _, tok := range toks {
		texts = append(texts, tok.text)
	}
	if got := strings.Join(texts, " "); got != "a [ 1 .. 2 ]" {
		t.Fatalf("tokens = %q", got)
	}
}

func TestLexErrors(t *testing.T) {
	tests := []struct{ name, src, want string }{
		{"block comment", "a\n/* x", "line 2"},
		{"nested comment", "a\n/+ /+ +/", "line 2"},
		{"string", "a\nb\n\"x", "line 3"},
		{"wysiwyg", "`x", "line 1"},
		{"delimited", "\nq\"(x", "line 2"},
		{"heredoc", "q\"EOS\nx\n", "line 1"},
		{"token string", "\n\nq{ x", "line 3"},
		{"char", "'a\n'", "line 1"},
		{"interpolated", "\ni\"$(x", "line 2"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			_, err := Analyze([]byte(tc.src))
			if err == nil || !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("err = %v, want it to mention %s", err, tc.want)
			}
		})
	}
}

func TestDefinesMainScope(t *testing.T) {
	tests := []struct {
		name string
		src  string
		want bool
	}{
		{"plain", "void main() {}", true},
		{"with args", "int main(string[] args) { return 0; }", true},
		{"extern C", "extern(C) int main() { return 0; }", true},
		{"version", "version (linux) { void main() {} }", true},
		{"version else", "version (a) {} else { void main() {} }", true},
		{"else version", "version (a) {} else version (b) { void main() {} }", true},
		{"debug", "debug { void main() {} }", true},
		{"static if", "static if (true) { void main() {} }", true},
		{"attribute block", "@safe { void main() {} }", true},
		{"extern block", "extern (C) { void main() {} }", true},
		{"version after import", "import std.stdio;\nversion (Posix) { void main() {} }", true},
		{"version after selective import", "import std.stdio : writeln;\nversion (Posix) { void main() {} }", true},
		{"static if on import expression", "static if (import(\"f\").length) { void main() {} }", true},
		{"struct after import", "import a;\nstruct S { void main() {} }", false},
		{"struct", "struct S { void main() {} }", false},
		{"class", "class C { void main() {} }", false},
		{"function body", "void f() { void main() {} }", false},
		{"template", "template T() { void main() {} }", false},
		{"version in struct", "struct S { version (a) { void main() {} } }", false},
		{"call", "void f() { main(); }", false},
		{"unittest", "unittest { void main() {} }", false},
		{"none", "void other() {}", false},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			info, err := Analyze([]byte(tc.src))
			if err != nil {
				t.Fatal(err)
			}
			if info.DefinesMain != tc.want {
				t.Fatalf("DefinesMain = %v, want %v", info.DefinesMain, tc.want)
			}
		})
	}
}
