package python

import (
	"reflect"
	"testing"
)

func TestParseImports(t *testing.T) {
	cases := []struct {
		name   string
		source string
		want   []string
	}{
		{"empty", "", nil},
		{"plain", "import foo\n", []string{"foo"}},
		{"stdlib", "import os\n", []string{"os"}},
		{"dotted", "import foo.bar\n", []string{"foo"}},
		{"list", "import a, b.c as d, e\n", []string{"a", "b", "e"}},
		{"asPrefix", "import assignment\n", []string{"assignment"}},
		{"spacedDot", "import a . b\n", []string{"a"}},
		{"trailingDot", "import a . \n", nil}, // malformed: no import node
		{"starSkipped", "import a, *\n", []string{"a"}},
		{"semicolons", "import a; import b\n", []string{"a", "b"}},
		{"continuation", "import a, \\\n b\n", []string{"a", "b"}},
		{"noTrailingNewline", "import foo", []string{"foo"}},
		{"trailingSpace", "import a as b ", []string{"a"}},
		{"from", "from x import y\n", []string{"x"}},
		{"fromDotted", "from x.y import c, d\n", []string{"x"}},
		{"fromParen", "from x import (a,\n b)\n", []string{"x"}},
		{"fromStar", "from x import *\n", []string{"x"}},
		{"fromNoNewline", "from foo import bar", []string{"foo"}},
		{"fromNoImport", "from foo bar\n", nil}, // malformed
		{"fromContinuation", "from foo \\\n import bar\n", []string{"foo"}},
		{"namesContinuation", "from foo import \\\n bar\n", []string{"foo"}},
		{"unclosedParen", "from foo import (a,\n b", nil}, // malformed
		{"semicolonParen", "from foo import (a; b)\n", []string{"foo"}},
		{"relativeBare", "from . import foo\n", nil},
		{"relativeModule", "from .foo import bar\n", nil},
		{"fromNewline", "from\n", nil},
		{"fromEOF", "from", nil},
		{"fromSemicolon", "from; import os\n", []string{"os"}},
		{"raiseFrom", "try:\n    pass\nexcept E as exc:\n    raise V() from exc\n", nil},
		{"indentedFrom", "if x:\n    from foo import bar\n", []string{"foo"}},
		{"noImports", "x = 1\n", nil},
		{"comment", "# import foo\n", nil},
		{"trailingComment", "import a # import b\n", []string{"a"}},
		{"doubleString", "\"import foo\"\nimport y\n", []string{"y"}},
		{"tripleSingle", "'''import foo'''\nimport y\n", []string{"y"}},
		{"tripleDouble", "\"\"\"import foo\"\"\"\nimport y\n", []string{"y"}},
		{"escapedQuote", "\"a\\\"x\"\nimport y\n", []string{"y"}},
		{"tripleEscape", "\"\"\"a\\nb\"\"\"\nimport z\n", []string{"z"}},
		{"unterminatedLine", "\"abc\nimport y\n", []string{"y"}},
		{"unterminatedEOF", "\"abc", nil},
		{"unterminatedTriple", "\"\"\"abc", nil},
		{"bareIdentifier", "x = import_module\n", nil},
		{"dynamic", "from importlib import import_module\nimport_module(\"plug\")\n", []string{"importlib", "plug"}},
		{"dynamicAttr", "import mod\nmod.import_module('plug.sub')\n", []string{"mod", "plug"}},
		{"dynamicTriple", "import_module(\"\"\"plug\"\"\")\n", []string{"plug"}},
		{"dynamicNewlines", "import_module(\n\"plug\"\n)\n", []string{"plug"}},
		{"dynamicComputed", "import_module(name)\n", nil},
		{"dynamicUnterminated", "import_module(\"abc)\n", nil},
		{"dynamicTripleUnterminated", "import_module(\"\"\"abc)\n", nil},
		{"dynamicEmpty", "import_module(\"\")\n", nil},
		{"dynamicRelative", "import_module(\".hidden\")\n", nil},
	}
	for _, tc := range cases {
		if got := ParseImports([]byte(tc.source)); !reflect.DeepEqual(got, tc.want) {
			t.Errorf("%s: ParseImports = %q, want %q", tc.name, got, tc.want)
		}
	}
}

func TestIsStdLib(t *testing.T) {
	if !IsStdLib("os") || !IsStdLib("sys") || !IsStdLib("importlib") {
		t.Error("stdlib identities not recognized")
	}
	if IsStdLib("helper") || IsStdLib("") || IsStdLib("os.path") {
		t.Error("non-stdlib identities recognized as stdlib")
	}
}
