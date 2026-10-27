package rust

import (
	"errors"
	"fmt"
	"strings"
	"testing"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

func mustParse(t *testing.T, path, src string) *FileFacts {
	t.Helper()
	facts, err := ParseFacts(path, []byte(src))
	if err != nil {
		t.Fatalf("ParseFacts(%s) unexpected error: %v", path, err)
	}
	return facts
}

func usePaths(facts *FileFacts) []string {
	out := make([]string, 0, len(facts.Uses))
	for _, u := range facts.Uses {
		out = append(out, u.Path)
	}
	return out
}

func TestParseBasicLib(t *testing.T) {
	facts := mustParse(t, "src/lib.rs", `
pub mod parser;
mod layout;
use std::collections::HashMap;
use crate::naming::Normalize;
extern crate serde;
`)
	if len(facts.Modules) != 2 {
		t.Fatalf("modules = %+v", facts.Modules)
	}
	if facts.Modules[0].Name != "parser" || facts.Modules[1].Name != "layout" {
		t.Errorf("modules = %+v", facts.Modules)
	}
	for _, m := range facts.Modules {
		if m.Inline || m.CfgTest {
			t.Errorf("module %+v should be a plain file module", m)
		}
	}
	got := usePaths(facts)
	want := []string{"std::collections::HashMap", "crate::naming::Normalize"}
	if strings.Join(got, ",") != strings.Join(want, ",") {
		t.Errorf("uses = %v, want %v", got, want)
	}
	if len(facts.Externs) != 1 || facts.Externs[0].Name != "serde" {
		t.Errorf("externs = %+v", facts.Externs)
	}
	if facts.HasTestAttr || facts.HasCfgTest {
		t.Error("plain lib must not report test markers")
	}
}

func TestParseInertRegions(t *testing.T) {
	facts := mustParse(t, "src/lib.rs", `
pub mod real;
const S: &str = "#[test] fn fake() {}";
const C: char = '\'';
fn live<'a>(x: &'a str) -> &'a str { x }
macro_rules! gen {
    () => {
        #[test]
        fn generated() {}
        use hidden::dep;
        mod hidden;
    };
}
`)
	if len(facts.Modules) != 1 || facts.Modules[0].Name != "real" {
		t.Errorf("modules = %+v, want only [real]", facts.Modules)
	}
	if len(facts.Uses) != 0 {
		t.Errorf("uses = %+v, macro body must be inert", facts.Uses)
	}
	if facts.HasTestAttr {
		t.Error("comment/string/macro #[test] must not trigger a unit-test target")
	}
}

func TestParseBlockCommentNested(t *testing.T) {
	facts := mustParse(t, "src/lib.rs", `
pub mod visible;
`)
	if len(facts.Modules) != 1 || facts.Modules[0].Name != "visible" {
		t.Errorf("modules = %+v", facts.Modules)
	}
	if len(facts.Uses) != 0 {
		t.Errorf("uses = %+v, block comment must be inert", facts.Uses)
	}
}

func TestParseUseGroupsAndAliases(t *testing.T) {
	facts := mustParse(t, "src/lib.rs", `
use std::{collections::HashMap, fmt::{self, Display}};
use serde::Serialize as Ser;
use crate::inner::*;
use super::sibling::Item;
`)
	got := usePaths(facts)
	want := []string{
		"std::collections::HashMap",
		"std::fmt",
		"std::fmt::Display",
		"serde::Serialize",
		"crate::inner::*",
		"super::sibling::Item",
	}
	if strings.Join(got, ",") != strings.Join(want, ",") {
		t.Errorf("uses = %v, want %v", got, want)
	}
	if !facts.Uses[4].Glob {
		t.Errorf("crate::inner::* must be marked as a glob: %+v", facts.Uses[4])
	}
}

func TestParseExternCrateAlias(t *testing.T) {
	facts := mustParse(t, "src/lib.rs", `
extern crate serde_json as json;
extern "C" {
    fn opaque() -> u32;
}
`)
	if len(facts.Externs) != 1 {
		t.Fatalf("externs = %+v", facts.Externs)
	}
	if facts.Externs[0].Name != "serde_json" || facts.Externs[0].As != "json" {
		t.Errorf("externs = %+v", facts.Externs)
	}
}

func TestParsePathAttr(t *testing.T) {
	facts := mustParse(t, "src/lib.rs", `
#[path = "custom/location.rs"]
pub mod renamed;
#[path = "../shared.rs"]
mod shared;
`)
	if len(facts.Modules) != 2 {
		t.Fatalf("modules = %+v", facts.Modules)
	}
	if facts.Modules[0].Path != "custom/location.rs" {
		t.Errorf("modules[0] = %+v", facts.Modules[0])
	}
	if facts.Modules[1].Path != "../shared.rs" {
		t.Errorf("modules[1] = %+v", facts.Modules[1])
	}
}

func TestParseTestRegions(t *testing.T) {
	facts := mustParse(t, "src/lib.rs", `
pub mod live;
#[cfg(test)]
mod tests;
use std::fmt::Debug;
#[cfg(test)]
use test_only::Helper;
#[test]
fn passes() {}
fn other() {}
`)
	if !facts.HasTestAttr {
		t.Error("expected HasTestAttr from #[test] fn")
	}
	if !facts.HasCfgTest {
		t.Error("expected HasCfgTest from #[cfg(test)] items")
	}
	testMods := 0
	for _, m := range facts.Modules {
		if m.CfgTest {
			testMods++
			if m.Name != "tests" {
				t.Errorf("unexpected test module %+v", m)
			}
		}
	}
	if testMods != 1 {
		t.Errorf("modules = %+v, want exactly one test module", facts.Modules)
	}
	testUses := 0
	for _, u := range facts.Uses {
		if u.CfgTest {
			testUses++
			if u.Path != "test_only::Helper" {
				t.Errorf("unexpected test use %+v", u)
			}
		}
	}
	if testUses != 1 {
		t.Errorf("uses = %+v, want exactly one test-only use", facts.Uses)
	}
}

func TestParseInlineModPropagatesTestScope(t *testing.T) {
	facts := mustParse(t, "src/lib.rs", `
pub mod live;
#[cfg(test)]
mod inner {
    use scoped::Only;
    #[test]
    fn it_works() {}
}
`)
	if !facts.HasTestAttr || !facts.HasCfgTest {
		t.Errorf("expected test markers, got %+v", facts)
	}
	for _, u := range facts.Uses {
		if !u.CfgTest {
			t.Errorf("use inside #[cfg(test)] inline mod must be test-only: %+v", u)
		}
	}
}

func TestParseInlineModParent(t *testing.T) {
	cases := []struct {
		src, name, parent string
		fromFileDir       bool
	}{
		{"mod a { mod b { mod c; } }\n", "c", "a/b", false},
		{"mod top;\n", "top", "", false},
		{"#[path = \"d\"] mod x { mod y; }\n", "y", "d", true},
		{"mod o { #[path = \"d\"] mod x { mod y; } }\n", "y", "o/d", false},
		{"mod o { #[path = \"/abs\"] mod x { mod y; } }\n", "y", "/abs", false},
	}
	for _, c := range cases {
		facts := mustParse(t, "src/lib.rs", c.src)
		var got *ModuleDecl
		for i := range facts.Modules {
			if facts.Modules[i].Name == c.name {
				got = &facts.Modules[i]
			}
		}
		if got == nil {
			t.Errorf("src %q: no module %s in %+v", c.src, c.name, facts.Modules)
			continue
		}
		if got.Parent != c.parent || got.ParentFromFileDir != c.fromFileDir {
			t.Errorf("src %q: %s = %+v, want Parent %q ParentFromFileDir %v", c.src, c.name, *got, c.parent, c.fromFileDir)
		}
	}
}

func TestParseModScope(t *testing.T) {
	type modWant struct {
		name, scope, parent string
	}
	type useWant struct {
		path, scope string
		cfgTest     bool
	}
	cases := []struct {
		src  string
		mods []modWant
		uses []useWant
	}{
		{
			src:  "mod a { mod b { mod c; use x::Y; } }\nuse top::Z;\n",
			mods: []modWant{{"a", "", ""}, {"b", "a", "a"}, {"c", "a/b", "a/b"}},
			uses: []useWant{{"x::Y", "a/b", false}, {"top::Z", "", false}},
		},
		{
			src:  "#[path = \"d\"] mod x { mod y; use y::T; }\n",
			mods: []modWant{{"x", "", ""}, {"y", "x", "d"}},
			uses: []useWant{{"y::T", "x", false}},
		},
		{
			src:  "#[cfg(test)] mod tests { use tempfile::tempdir; }\n",
			mods: []modWant{{"tests", "", ""}},
			uses: []useWant{{"tempfile::tempdir", "tests", true}},
		},
		{
			src:  "#[cfg(test)] mod tests { mod helpers; struct F { x: u8 } impl F { } use helpers::Y; }\nuse after::Z;\n",
			mods: []modWant{{"tests", "", ""}, {"helpers", "tests", "tests"}},
			uses: []useWant{{"helpers::Y", "tests", true}, {"after::Z", "", false}},
		},
		{
			src:  "mod a { struct S { x: u8 } mod b; }\nmod c;\n",
			mods: []modWant{{"a", "", ""}, {"b", "a", "a"}, {"c", "", ""}},
		},
	}
	for _, c := range cases {
		facts := mustParse(t, "src/lib.rs", c.src)
		var mods []modWant
		for _, m := range facts.Modules {
			mods = append(mods, modWant{m.Name, m.Scope, m.Parent})
		}
		var uses []useWant
		for _, u := range facts.Uses {
			uses = append(uses, useWant{u.Path, u.Scope, u.CfgTest})
		}
		if !sameElements(mods, c.mods) {
			t.Errorf("src %q: modules = %+v, want %+v", c.src, mods, c.mods)
		}
		if !sameElements(uses, c.uses) {
			t.Errorf("src %q: uses = %+v, want %+v", c.src, uses, c.uses)
		}
	}
}

func sameElements[T comparable](got, want []T) bool {
	if len(got) != len(want) {
		return false
	}
	seen := make(map[T]int, len(want))
	for _, w := range want {
		seen[w]++
	}
	for _, g := range got {
		if seen[g] == 0 {
			return false
		}
		seen[g]--
	}
	return true
}

func TestParseInnerCfgTest(t *testing.T) {
	facts := mustParse(t, "src/tests.rs", "#![cfg(test)]\nuse helper::Util;\n")
	if !facts.InnerCfgTest {
		t.Error("expected InnerCfgTest for #![cfg(test)]")
	}
}

func TestParseAmbiguousCfgFails(t *testing.T) {
	for _, tc := range []struct {
		src       string
		line, col int
		attr      string
	}{
		{"#[cfg(any(test, feature = \"x\"))]\nmod maybe;\n", 1, 1, "#[cfg(any(test, feature = \"x\"))]"},
		{"#[cfg(all(test, unix))]\nuse maybe::Thing;\n", 1, 1, "#[cfg(all(test, unix))]"},
		{"#[cfg_attr(test, ignore)]\nfn flaky() {}\n", 1, 1, "#[cfg_attr(test, ignore)]"},
		{"#[cfg(not(test))]\nmod also_mentions;\n", 1, 1, "#[cfg(not(test))]"},
		{"// c\nconst S: &str = \"a\nb\";\nfn a() {}\n\n#[cfg(any(target_os = \"linux\", test))]\nfn b() {}\n", 6, 1, "#[cfg(any(target_os = \"linux\", test))]"},
		{"mod x {\n    #[test]\n    #[cfg(any(unix, test))]\n    fn b() {}\n}\n", 3, 5, "#[cfg(any(unix, test))]"},
		{"const S: &str = \"é\"; #[cfg(all(test, unix))] fn b() {}\n", 1, 23, "#[cfg(all(test, unix))]"},
	} {
		_, err := ParseFacts("src/lib.rs", []byte(tc.src))
		var amb *AmbiguousError
		if !errors.As(err, &amb) {
			t.Errorf("src %q: expected ambiguous-context error, got %v", tc.src, err)
			continue
		}
		if amb.Path != "src/lib.rs" || amb.Line != tc.line || amb.Col != tc.col {
			t.Errorf("src %q: got %s:%d:%d want %d:%d", tc.src, amb.Path, amb.Line, amb.Col, tc.line, tc.col)
		}
		if got := tc.src[amb.StartByte:amb.EndByte]; got != tc.attr {
			t.Errorf("src %q: span %q want %q", tc.src, got, tc.attr)
		}
		want := fmt.Sprintf("src/lib.rs:%d:%d: ambiguous test context", tc.line, tc.col)
		if !strings.Contains(err.Error(), want) {
			t.Errorf("src %q: message %v lacks %q", tc.src, err, want)
		}
	}
}

func TestParseNonTestCfgIsUnconditional(t *testing.T) {
	facts := mustParse(t, "src/lib.rs", `
#[cfg(feature = "fast")]
use accel::Turbo;
#[cfg(unix)]
mod platform;
`)
	if len(facts.Uses) != 1 || facts.Uses[0].CfgTest {
		t.Errorf("uses = %+v, feature-gated use must be a production edge", facts.Uses)
	}
	if len(facts.Modules) != 1 || facts.Modules[0].CfgTest {
		t.Errorf("modules = %+v, cfg-gated mod must be a production module", facts.Modules)
	}
}

func TestParseRawStringsInert(t *testing.T) {
	facts := mustParse(t, "src/lib.rs", `
const RAW: &str = r#"use fake::Dep; mod fake;"#;
const BYTES: &[u8] = b"use nope::X;";
pub mod real;
`)
	if len(facts.Modules) != 1 || facts.Modules[0].Name != "real" {
		t.Errorf("modules = %+v", facts.Modules)
	}
	if len(facts.Uses) != 0 {
		t.Errorf("uses = %+v, raw strings must be inert", facts.Uses)
	}
}

func TestParsePubRestrictedMod(t *testing.T) {
	facts := mustParse(t, "src/lib.rs", `
pub(crate) mod internal;
pub(in crate::parent) mod scoped;
`)
	if len(facts.Modules) != 2 {
		t.Fatalf("modules = %+v", facts.Modules)
	}
	if facts.Modules[0].Name != "internal" || facts.Modules[1].Name != "scoped" {
		t.Errorf("modules = %+v", facts.Modules)
	}
}

func TestNormalizeIntegration(t *testing.T) {
	facts := mustParse(t, "src/lib.rs", "mod user_profile;\n")
	stem, err := common.Normalize(facts.Modules[0].Name)
	if err != nil {
		t.Fatal(err)
	}
	if stem != "user_profile" {
		t.Errorf("stem = %q", stem)
	}
}

func TestParseMalformedItems(t *testing.T) {
	cases := []string{
		"mod;",
		"mod name",
		"mod name = bad;",
		"use thing",
		"use ;",
		"extern crate ;",
		"extern crate name as ;",
	}
	for _, source := range cases {
		if _, err := ParseFacts("broken.rs", []byte(source)); err == nil {
			t.Errorf("expected parse failure for %q", source)
		}
	}
}

func TestExternAliasImportsCrateName(t *testing.T) {
	tree := map[string]*FileFacts{"src/lib.rs": mustParse(t, "src/lib.rs", "extern crate serde_json as json;\nuse json::Value;\n")}
	got := importsForEdition(tree, "2021", "src/lib.rs")
	if len(got.production) != 1 || got.production[0] != "serde_json" {
		t.Errorf("production = %v, want [serde_json]", got.production)
	}
}

func TestParseTestAttributesOnAllItems(t *testing.T) {
	facts := mustParse(t, "tests.rs", `
#[cfg(test)] use test_use::Thing;
#[cfg(test)] extern crate test_extern;
#[cfg(test)] mod file_tests;
mod inline {
    #[test]
    fn nested() {}
}
`)
	if !facts.HasCfgTest || !facts.HasTestAttr || !facts.Uses[0].CfgTest || !facts.Externs[0].CfgTest || !facts.Modules[0].CfgTest {
		t.Errorf("test attributes = %+v", facts)
	}
}

func TestParseAmbiguousAttributesOnAllItems(t *testing.T) {
	for _, source := range []string{
		"#[cfg(any(test, unix))] use maybe::Thing;",
		"#[cfg(any(test, unix))] extern crate maybe;",
		"mod inline { #[cfg(any(test, unix))] use maybe::Thing; }",
		"fn f() { #[cfg(any(test, unix))] use m::T; }",
	} {
		if _, err := ParseFacts("ambiguous.rs", []byte(source)); err == nil {
			t.Errorf("ambiguous item accepted: %s", source)
		}
	}
}

func TestParseFunctionAndExternEdges(t *testing.T) {
	facts := mustParse(t, "edge.rs", "extern crate plain;\nfn declaration();\nfn unclosed() {\n")
	if len(facts.Externs) != 1 || facts.Externs[0].Name != "plain" {
		t.Errorf("externs = %+v", facts.Externs)
	}
	if _, err := ParseFacts("edge.rs", []byte("extern crate plain next;\n")); err == nil {
		t.Error("malformed extern crate accepted")
	}
}

func TestParseFnBodyUses(t *testing.T) {
	type use struct {
		path  string
		test  bool
		scope string
	}
	cases := []struct {
		name string
		src  string
		want []use
	}{
		{"plain", "fn f() { use serde_json::Value; }", []use{{"serde_json::Value", false, ""}}},
		{"test attr", "#[test] fn t() { use tempfile::tempdir; }", []use{{"tempfile::tempdir", true, ""}}},
		{"cfg test", "#[cfg(test)] fn h() { use x::Y; }", []use{{"x::Y", true, ""}}},
		{"nested fn", "#[test] fn t() { fn inner() { use a::B; } }", []use{{"a::B", true, ""}}},
		{"mod in fn", "fn f() { mod m { use d::E; } }", []use{{"d::E", false, "m"}}},
		{"impl method", "impl S { fn m(&self) { use e::F; } }", []use{{"e::F", false, ""}}},
		{"fn pointer cast", "#[test] fn t() { let g = if c { a as fn() } else { b as fn() }; }\nuse prod::X;", []use{{"prod::X", false, ""}}},
		{"fn pointer field", "struct S { cb: fn(u8) }\nuse foo::Bar;", []use{{"foo::Bar", false, ""}}},
		{"array sig", "#[test] fn t(x: [u8; 4]) { use z::Y; }", []use{{"z::Y", true, ""}}},
		{"macro group", "fn e() { quote! { use serde::Serialize; } let a = b != c; }", nil},
	}
	for _, c := range cases {
		facts := mustParse(t, "body.rs", c.src)
		if len(facts.Uses) != len(c.want) {
			t.Errorf("%s: uses = %+v", c.name, facts.Uses)
			continue
		}
		for i, w := range c.want {
			u := facts.Uses[i]
			if u.Path != w.path || u.CfgTest != w.test || u.Scope != w.scope {
				t.Errorf("%s: use %d = %+v, want %+v", c.name, i, u, w)
			}
		}
	}
	if facts := mustParse(t, "inner.rs", "fn f() { #![cfg(test)] }"); facts.InnerCfgTest {
		t.Errorf("inner cfg(test) in fn body set InnerCfgTest")
	}
	if facts := mustParse(t, "t.rs", "#[test] fn t() { }"); !facts.HasTestAttr {
		t.Errorf("HasTestAttr unset")
	}
}


func TestParseBrIdentifiersAndLeadingColons(t *testing.T) {
	facts := mustParse(t, "src/lib.rs", "mod bridge;\nconst X: &[u8] = br\"x\"; use brotli::X;\nuse ::serde::Value;\nextern crate serde_json as json;\n")
	if len(facts.Modules) != 1 || facts.Modules[0].Name != "bridge" {
		t.Errorf("modules = %+v, want bridge", facts.Modules)
	}
	roots := map[string]bool{}
	for _, u := range facts.Uses {
		roots[useRoot(u.Path)] = true
	}
	if !roots["brotli"] || !roots["serde"] || len(roots) != 2 {
		t.Errorf("use roots = %v, want brotli and serde", roots)
	}
	if len(facts.Externs) != 1 || facts.Externs[0].Name != "serde_json" {
		t.Errorf("externs = %+v, want serde_json", facts.Externs)
	}
}

func TestParserLiteralEdges(t *testing.T) {
	if _, err := ParseFacts("a.rs", []byte("const A: &str = \"unterminated")); err == nil {
		t.Error("unterminated string accepted")
	}
	for _, source := range []string{
		"const A: &str = r###\"use raw::X;\"###;",
		"const A: char = '\\'';",
		"const A: char = '\\u{41}';",
		"const A: u8 = b'x';",
		"const A: &[u8] = br\"use bytes::X;\";",
		"const A: &str = \"a\\\\\"; use real::X;",
		"const A: char = 'é';",
		"fn f<'a>(x: &'a str) -> &'a str { x }",
	} {
		facts := mustParse(t, "lit.rs", source)
		for _, u := range facts.Uses {
			if u.Path != "real::X" {
				t.Errorf("source %q: literal leaked use %q", source, u.Path)
			}
		}
	}
}

func TestParseAttributesAndUseEdges(t *testing.T) {
	facts := mustParse(t, "edge.rs", `
#![cfg(test)]
#[other]
pub(crate) mod inline { use foo::{self, nested::{Item}}; }
extern "C" { fn foreign(); }
fn declaration();
`)
	if !facts.InnerCfgTest || len(facts.Modules) != 1 || len(facts.Uses) != 2 {
		t.Errorf("facts = %+v", facts)
	}
	facts = mustParse(t, "edge.rs", "#[other(a = 1, pathname = 2)]\nmod m;\n#[path]\nmod n;\n#[path = bare]\nmod o;\n#[pathology = \"x\"]\nmod p;\n")
	for _, m := range facts.Modules {
		if m.Path != "" {
			t.Errorf("module %+v must not carry a path override", m)
		}
	}
	for _, source := range []string{"#[cfg(feature = \"test\")] mod a;", "#[cfg(unix)] mod a;", "#[derive(Debug)] struct S;"} {
		if facts := mustParse(t, "attr.rs", source); facts.HasCfgTest {
			t.Errorf("source %q marked cfg(test)", source)
		}
	}
	if facts := mustParse(t, "attr.rs", "#[ cfg ( test ) ] mod a;"); !facts.HasCfgTest {
		t.Error("spaced cfg(test) not recognized")
	}
	if _, err := ParseFacts("attr.rs", []byte("#[tokio::test]\nasync fn t() {}\n")); err == nil {
		t.Error("path attribute ending in test must be ambiguous")
	}
}

func TestParserScannerEdges(t *testing.T) {
	for _, source := range []string{"# nope", "#[unterminated", "#!", "42"} {
		if _, err := ParseFacts("scan.rs", []byte(source)); err == nil {
			t.Errorf("invalid source accepted: %q", source)
		}
	}
	if facts := mustParse(t, "scan.rs", "#![ cfg ( test ) ]\n"); !facts.InnerCfgTest {
		t.Error("spaced inner attribute not recognized")
	}
	if facts := mustParse(t, "scan.rs", "pub(crate) fn f() {}\nuse a::B;\n"); len(facts.Uses) != 1 {
		t.Errorf("uses = %+v", facts.Uses)
	}
}

func TestUseAndAttributeNestedEdges(t *testing.T) {
	facts := mustParse(t, "nested.rs", "use {foo, bar};\nuse foo::{bar::{baz, qux as q}, self as me, *};\n#[nested[a]]\nmod m;\n")
	want := "foo,bar,foo::bar::baz,foo::bar::qux,foo,foo::*"
	if got := strings.Join(usePaths(facts), ","); got != want {
		t.Errorf("uses = %s, want %s", got, want)
	}
	for _, source := range []string{"use foo::{bar;", "use foo::{;", "use ;"} {
		if _, err := ParseFacts("nested.rs", []byte(source)); err == nil {
			t.Errorf("malformed use accepted: %q", source)
		}
	}
}

func TestParserCoverageClosure(t *testing.T) {
	facts := mustParse(t, "closure.rs", "#[foo([bar])]\nmod m;\n")
	if len(facts.Modules) != 1 || facts.Modules[0].Name != "m" {
		t.Errorf("modules = %+v", facts.Modules)
	}
	if _, err := ParseFacts("closure.rs", []byte("# lone\nmod m;\n")); err == nil {
		t.Error("lone hash accepted")
	}
	if _, err := ParseFacts("closure.rs", []byte("extern C foo;\n")); err == nil {
		t.Error("malformed extern accepted")
	}
	if _, err := ParseFacts("closure.rs", []byte("#[doc = \"a\\\\b\"] #[cfg(any(test, unix))] mod m;")); err == nil {
		t.Error("escaped string hid the test token")
	}
}

func TestPrivateScannerBranches(t *testing.T) {
	facts := mustParse(t, "branch.rs", "#[path = \"a\\\\b.rs\"]\nmod x;\n#[path = \"q\\\"uote\"]\nmod y;\n")
	if len(facts.Modules) != 2 || facts.Modules[0].Path != `a\b.rs` || facts.Modules[1].Path != `q"uote` {
		t.Errorf("escaped paths = %+v", facts.Modules)
	}
	facts = mustParse(t, "branch.rs", "#[cfg(test)]\nfn declaration();\n")
	if !facts.HasTestAttr {
		t.Error("cfg(test) function signature not marked")
	}
	facts = mustParse(t, "branch.rs", "#[cfg(test)] extern crate name;\n#[cfg(test)] use body;\n")
	if !facts.HasCfgTest || !facts.Externs[0].CfgTest || !facts.Uses[0].CfgTest {
		t.Errorf("direct test items = %+v", facts)
	}
}

func TestParseTopLevelMacroIsInert(t *testing.T) {
	facts := mustParse(t, "src/lib.rs", "cfg_if! { use x; mod hidden; }\nuse real::Dep;\n")
	if len(facts.Modules) != 0 || len(facts.Uses) != 1 || facts.Uses[0].Path != "real::Dep" {
		t.Errorf("facts = %+v", facts)
	}
}

func TestParseLexicalForms(t *testing.T) {
	cases := []struct {
		name, src string
		uses      []string
	}{
		{"shebang", "#!/usr/bin/env run\nuse a::B;\n", []string{"a::B"}},
		{"raw identifier", "use r#type::Thing;\nmod r#mod;\n", []string{"r#type::Thing"}},
		{"nested block comment", "/* outer /* use hidden::X; */ use hidden::Y; */\nuse real::Z;\n", []string{"real::Z"}},
		{"line comment", "// use hidden::X;\nuse real::Z; // trailing\n", []string{"real::Z"}},
		{"doc comment", "/// use hidden::X;\nfn f() {}\nuse real::Z;\n", []string{"real::Z"}},
		{"closure", "fn f() { let c = || { use inner::A; }; }\n", []string{"inner::A"}},
		{"if", "fn f() { if c { use inner::B; } else { use inner::C; } }\n", []string{"inner::B", "inner::C"}},
		{"comment inside path", "use a /* c */ :: b;\n", []string{"a::b"}},
		{"spaced path", "use a :: b ::\n c;\n", []string{"a::b::c"}},
	}
	for _, c := range cases {
		facts := mustParse(t, "lex.rs", c.src)
		if got := strings.Join(usePaths(facts), ","); got != strings.Join(c.uses, ",") {
			t.Errorf("%s: uses = %s, want %s", c.name, got, strings.Join(c.uses, ","))
		}
	}
}

func TestParseSyntaxErrorPositions(t *testing.T) {
	cases := []struct {
		src       string
		line, col int
		contains  string
	}{
		{"mod;", 1, 1, "expected module name after `mod`"},
		{"use missing", 1, 12, "unterminated"},
		{"const S: &str = \"é\"; mod ;", 1, 23, "expected module name"},
		{"fn a() {}\n\nstruct = ;\n", 3, 1, "syntax error"},
	}
	for _, c := range cases {
		_, err := ParseFacts("src/lib.rs", []byte(c.src))
		if err == nil {
			t.Errorf("src %q: expected error", c.src)
			continue
		}
		prefix := fmt.Sprintf("rust: src/lib.rs:%d:", c.line)
		if !strings.HasPrefix(err.Error(), prefix) || !strings.Contains(err.Error(), c.contains) {
			t.Errorf("src %q: error %q, want prefix %q containing %q", c.src, err, prefix, c.contains)
		}
		if want := fmt.Sprintf(":%d:%d:", c.line, c.col); !strings.Contains(err.Error(), want) {
			t.Errorf("src %q: error %q lacks position %s", c.src, err, want)
		}
	}
}

func TestParseToleratesOlderEditionKeywords(t *testing.T) {
	facts := mustParse(t, "old.rs", "pub fn gate() -> i32 {\n    let async = 1;\n    async.max(0)\n}\nuse real::Dep;\n")
	if len(facts.Uses) != 1 || facts.Uses[0].Path != "real::Dep" {
		t.Errorf("uses = %+v", facts.Uses)
	}
	if _, err := ParseFacts("old.rs", []byte("fn f() { use broken::; }")); err == nil {
		t.Error("malformed use inside a function body accepted")
	}
}

const truncationSample = `#![cfg(test)]
#[path = "x.rs"]
pub(crate) mod a { use b::{c as d, e::*}; extern crate f as g; enum E { V } }
fn h() { let c = || { use i::j; }; }
cfg_if! { use hidden; }
const S: &str = "é\n";
`

func TestParseTruncatedInputsDoNotPanic(t *testing.T) {
	for i := 0; i <= len(truncationSample); i++ {
		_, _ = ParseFacts("trunc.rs", []byte(truncationSample[:i]))
	}
}

func FuzzParseFacts(f *testing.F) {
	for _, seed := range []string{
		truncationSample,
		"mod a;",
		"#[cfg(test)] mod tests { use x::Y; }",
		"use std::{fmt::{self, Display}, io};",
		"#[cfg(any(test, unix))] fn f() {}",
		"fn f() { use a::B; }",
	} {
		f.Add([]byte(seed))
	}
	f.Fuzz(func(t *testing.T, src []byte) {
		_, _ = ParseFacts("fuzz.rs", src)
	})
}
