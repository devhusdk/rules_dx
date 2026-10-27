package perl

import (
	"reflect"
	"testing"
)

func TestParseImports(t *testing.T) {
	cases := []struct {
		name string
		src  string
		want []string
	}{
		{"use bareword", "use Foo::Bar;\n", []string{"Foo::Bar"}},
		{"use with version and list", "use Foo::Bar 1.2 qw(a b);\n", []string{"Foo::Bar"}},
		{"use version", "use v5.36;\nuse 5.040;\n", nil},
		{"no loads the module", "no Old::Thing;\n", []string{"Old::Thing"}},
		{"require bareword", "require Req::One;\n", []string{"Req::One"}},
		{"require file", "require \"file.pl\";\nrequire $x;\n", nil},
		{"require version", "require 5.006;\n", nil},
		{"parent qw", "use parent qw(Base::B Base::C);\n", []string{"Base::C", "Base::B", "parent"}},
		{"parent string", "use parent 'Base::A';\n", []string{"Base::A", "parent"}},
		{"parent norequire", "use parent -norequire, 'Base::A';\n", []string{"parent"}},
		{"base list", "use base ('Base::A', \"Base::B\");\n", []string{"Base::A", "Base::B", "base"}},
		{"eval block guard", "eval { require Opt::Mod; 1 };\n", nil},
		{"eval string guard", "eval \"require Str::Mod\";\n", nil},
		{"eval use guard", "eval { require Opt::Mod; 1 } or do { require Fallback::Mod };\n", []string{"Fallback::Mod"}},
		{"begin block", "BEGIN { require Beg::Mod }\n", []string{"Beg::Mod"}},
		{"lazy require in sub", "sub f { require Lazy::Mod; }\n", []string{"Lazy::Mod"}},
		{"heredoc", "print <<EOT;\nuse In::Heredoc;\nEOT\nuse Real::One;\n", []string{"Real::One"}},
		{"pod", "=pod\n\nuse In::Pod;\n\n=cut\nuse Real::One;\n", []string{"Real::One"}},
		{"string", "my $s = \"use In::String;\";\n", nil},
		{"comment", "# use In::Comment;\n", nil},
		{"data section", "use Real::One;\n__END__\nuse After::End;\n", []string{"Real::One"}},
		{"data marker", "use Real::One;\n__DATA__\nuse After::Data;\n", []string{"Real::One"}},
		{"crlf", "use Foo::Bar;\r\nuse Baz;\r\n", []string{"Baz", "Foo::Bar"}},
		{"duplicates", "use Foo;\nuse Foo;\nrequire Foo;\n", []string{"Foo"}},
		{"truncated", "use Foo::Bar;\nuse Baz::Qux;\nsub broken {\n    my $x = (\n", []string{"Baz::Qux", "Foo::Bar"}},
	}
	for _, tc := range cases {
		got := ParseImports([]byte(tc.src))
		want := append([]string(nil), tc.want...)
		sortStrings(want)
		if len(got) == 0 && len(want) == 0 {
			continue
		}
		if !reflect.DeepEqual(got, want) {
			t.Errorf("%s: ParseImports = %v, want %v", tc.name, got, want)
		}
	}
}

func sortStrings(s []string) {
	for i := range s {
		for j := i + 1; j < len(s); j++ {
			if s[j] < s[i] {
				s[i], s[j] = s[j], s[i]
			}
		}
	}
}
