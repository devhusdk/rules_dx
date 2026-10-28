package php

import (
	"reflect"
	"testing"
)

func TestParse(t *testing.T) {
	cases := []struct {
		name         string
		src          string
		wantDeclares []string
		wantImports  []string
	}{
		{"declares in namespace", "<?php\nnamespace App\\Models;\nclass User {}\ninterface I {}\ntrait T {}\nenum E {}\n",
			[]string{`App\Models\E`, `App\Models\I`, `App\Models\T`, `App\Models\User`}, nil},
		{"global declarations", "<?php\nclass A {}\nfunction f() {}\nconst C = 1;\n",
			[]string{"A", "C", "f"}, nil},
		{"namespaced function and const", "<?php\nnamespace N;\nfunction f() {}\nconst C = 1;\n",
			[]string{`N\C`, `N\f`}, nil},
		{"braced namespaces", "<?php\nnamespace A { class X {} }\nnamespace B { class Y extends X {} }\nnamespace { class G {} }\n",
			[]string{`A\X`, `B\Y`, "G"}, []string{`B\X`}},
		{"sequential namespaces", "<?php\nnamespace A;\nclass X {}\nnamespace B;\nclass Y extends X {}\n",
			[]string{`A\X`, `B\Y`}, []string{`B\X`}},
		{"use and alias", "<?php\nnamespace N;\nuse Foo\\Bar as Baz;\nclass C extends Baz {}\n",
			[]string{`N\C`}, []string{`Foo\Bar`}},
		{"use leading backslash", "<?php\nuse \\Lead\\Name;\nnew Name();\n",
			nil, []string{`Lead\Name`}},
		{"use function and const", "<?php\nnamespace N;\nuse function Foo\\fn1;\nuse const Foo\\C1;\n",
			nil, []string{`Foo\C1`, `Foo\fn1`}},
		{"group use", "<?php\nuse Foo\\{A, B as BB, C\\D};\nuse function Foo\\{f1, f2};\nuse const Foo\\{K};\nnew A; new BB; new D;\n",
			nil, []string{`Foo\A`, `Foo\B`, `Foo\C\D`, `Foo\K`, `Foo\f1`, `Foo\f2`}},
		{"relative name resolves against namespace", "<?php\nnamespace App;\n$x = new Sub\\Thing();\n",
			nil, []string{`App\Sub\Thing`}},
		{"alias qualifies first segment", "<?php\nnamespace App;\nuse Lib\\Http;\n$x = new Http\\Client();\n",
			nil, []string{`Lib\Http\Client`}},
		{"alias used whole is imported", "<?php\nuse Lib\\Http;\n$x = new Http();\n",
			nil, []string{`Lib\Http`}},
		{"leading backslash", "<?php\nnamespace App;\n$x = new \\Exception();\n$y = \\Lib\\fn();\n",
			nil, []string{"Exception", `Lib\fn`}},
		{"namespace keyword", "<?php\nnamespace App;\nnamespace\\helper();\n",
			nil, []string{`App\helper`}},
		{"self static parent skipped", "<?php\nclass A extends B { function f(self $s): static { new static; new self; return parent::f(); } }\n",
			[]string{"A"}, []string{"B"}},
		{"primitive types skipped", "<?php\nfunction f(int $a, ?string $b, mixed $c, iterable $d): void {}\n",
			[]string{"f"}, nil},
		{"class positions", "<?php\nnamespace N;\nclass A extends B implements I, J {\n use T;\n function f(Typed $a, ?Opt $b, U|V $c): R { $x = S::m(); $y = P::$v; $z = K::CONST; $c = Cl::class; $i = $x instanceof Inst; try {} catch (E1 | E2 $e) {} }\n}\n#[Attr(1)]\nfunction g() {}\n",
			[]string{`N\A`, `N\g`}, []string{`N\Attr`, `N\B`, `N\Cl`, `N\E1`, `N\E2`, `N\I`, `N\Inst`, `N\J`, `N\K`, `N\Opt`, `N\P`, `N\R`, `N\S`, `N\T`, `N\Typed`, `N\U`, `N\V`}},
		{"constant name is not a class", "<?php\n$x = Foo::BAR;\n",
			nil, []string{"Foo"}},
		{"unqualified function call skipped", "<?php\nnamespace N;\nstrlen('x');\nSOME_CONST;\n",
			nil, nil},
		{"strings heredoc comments skipped", "<?php\n// new Comment\n/* new Block */\n$s = 'new Str\\Single'; $t = \"new Str\\Double\";\n$h = <<<EOT\nnew Doc\\Name\nEOT;\n$n = <<<'EOT'\nnew Now\\Name\nEOT;\nrequire 'a/b.php';\ninclude_once __DIR__ . '/c.php';\n",
			nil, nil},
		{"inline html skipped", "<?php class A {} ?>\n<html>new Html\\Name</html>\n",
			[]string{"A"}, nil},
		{"conditional declaration not indexed", "<?php\nif (true) { class Cond {} }\nif (!function_exists('f')) { function f() {} }\n",
			nil, nil},
		{"method and constants not indexed", "<?php\nclass A { const X = 1; function m() {} }\n",
			[]string{"A"}, nil},
		{"anonymous class", "<?php\n$o = new class extends Base implements Face {};\n",
			nil, []string{"Base", "Face"}},
		{"syntax error keeps earlier", "<?php\nuse Foo\\Bar;\nclass A extends Bar {}\nclass { ((\n",
			[]string{"A"}, []string{`Foo\Bar`}},
		{"utf8 bom", "\xef\xbb\xbf<?php\nnamespace N;\nclass A extends B {}\n",
			[]string{`N\A`}, []string{`N\B`}},
		{"modifiers", "<?php\nabstract class A extends B {}\nfinal class C {}\nreadonly class D {}\n",
			[]string{"A", "C", "D"}, []string{"B"}},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			gotD, gotI := Parse([]byte(c.src))
			if len(gotD) == 0 {
				gotD = nil
			}
			if len(gotI) == 0 {
				gotI = nil
			}
			if !reflect.DeepEqual(gotD, c.wantDeclares) {
				t.Errorf("declares = %q, want %q", gotD, c.wantDeclares)
			}
			if !reflect.DeepEqual(gotI, c.wantImports) {
				t.Errorf("imports = %q, want %q", gotI, c.wantImports)
			}
		})
	}
}
