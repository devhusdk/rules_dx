package dart

import (
	"reflect"
	"testing"
)

func TestParseDirectives(t *testing.T) {
	src := `// header comment
library foo;

import 'package:a/a.dart' as a show X hide Y;
import "dart:io" deferred as io;
import r'package:b/b.dart'
    if (dart.library.html) 'package:c/c.dart'
    if (dart.library.io) '''package:d/d.dart''';
/* import 'package:ignored/x.dart'; */
export 'src/x.dart' show Q;
part 'x.g.dart';
part of 'y.dart';

void main() { print('import "package:z/z.dart";'); }
`
	got, err := ParseDirectives([]byte(src))
	if err != nil {
		t.Fatal(err)
	}
	var uris []string
	for _, d := range got {
		uris = append(uris, d.Kind+" "+d.URI)
	}
	want := []string{
		"import package:a/a.dart",
		"import dart:io",
		"import package:b/b.dart",
		"import package:c/c.dart",
		"import package:d/d.dart",
		"export src/x.dart",
		"part x.g.dart",
	}
	if !reflect.DeepEqual(uris, want) {
		t.Errorf("got %q\nwant %q", uris, want)
	}
}

func TestParseDirectivesConditionalValue(t *testing.T) {
	got, err := ParseDirectives([]byte("import 'a.dart' if (dart.library.io == 'true') 'b.dart';\n"))
	if err != nil {
		t.Fatal(err)
	}
	if len(got) != 2 || got[0].URI != "a.dart" || got[1].URI != "b.dart" {
		t.Errorf("got %v", got)
	}
}

func TestParseDirectivesMalformed(t *testing.T) {
	for name, src := range map[string]string{
		"missing semicolon": "import 'package:a/a.dart'\nimport 'package:b/b.dart';\nvoid main() {}\n",
		"garbage":           "import ;;; @@\nvoid main() {}\n",
		"interpolated":      "import 'package:${x}/a.dart';\n",
	} {
		if _, err := ParseDirectives([]byte(src)); err == nil {
			t.Errorf("%s: want an error", name)
		}
	}
}

func TestParseDirectivesLeadingMetadata(t *testing.T) {
	cases := map[string]string{
		"teston":  "@TestOn('vm')\nlibrary;\nimport 'package:test/test.dart';\n",
		"timeout": "@Timeout(Duration(seconds: 1))\nimport 'package:test/test.dart';\nvoid main() {}\n",
		"strarg":  "@Foo('not/a/uri.dart')\nimport 'package:test/test.dart';\n",
	}
	for name, src := range cases {
		got, err := ParseDirectives([]byte(src))
		want := []Directive{{Kind: "import", URI: "package:test/test.dart"}}
		if err != nil || !reflect.DeepEqual(got, want) {
			t.Errorf("%s: got %v, %v; want %v", name, got, err, want)
		}
	}
}
