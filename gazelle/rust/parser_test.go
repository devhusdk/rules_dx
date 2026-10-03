package rust

import (
	"strings"
	"testing"

	"github.com/ralvik/rules_dx/gazelle/common"
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

func TestParseInnerCfgTest(t *testing.T) {
	facts := mustParse(t, "src/tests.rs", "#![cfg(test)]\nuse helper::Util;\n")
	if !facts.InnerCfgTest {
		t.Error("expected InnerCfgTest for #![cfg(test)]")
	}
}

func TestParseAmbiguousCfgFails(t *testing.T) {
	for _, src := range []string{
		"#[cfg(any(test, feature = \"x\"))]\nmod maybe;\n",
		"#[cfg(all(test, unix))]\nuse maybe::Thing;\n",
		"#[cfg_attr(test, ignore)]\nfn flaky() {}\n",
		"#[cfg(not(test))]\nmod also_mentions;\n",
	} {
		if _, err := ParseFacts("src/lib.rs", []byte(src)); err == nil {
			t.Errorf("src %q: expected ambiguous-context error", src)
		} else if !strings.Contains(err.Error(), "ambiguous test context") {
			t.Errorf("src %q: wrong error: %v", src, err)
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

func TestParserLiteralEdges(t *testing.T) {
	unterminated := cleanSource("const A: &str = \"unterminated")
	if strings.Contains(unterminated, "unterminated") {
		t.Errorf("unterminated string not blanked: %q", unterminated)
	}
	raw := cleanSource("const A: &str = r###\"raw\"###;")
	if strings.Contains(raw, "raw") {
		t.Errorf("raw string not blanked: %q", raw)
	}
	for _, source := range []string{"'", "''", "'ab'", "'\\u{41}'", "'\\n'"} {
		cleanSource(source)
	}
	for _, source := range []string{`b'x'`, `br"bytes"`, `r#missing`, `macro_rules! name {`, `macro name [use fake::X;]`} {
		cleanSource(source)
	}
	for _, source := range []string{`"a\\"b"`, "'\\u0041'", "'\\x'", "'a\n", "'é'"} {
		cleanSource(source)
	}
	if _, ok := matchBalanced([]byte("{missing"), 0); ok {
		t.Error("unbalanced group accepted")
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
	if got := expandUse(""); got != nil {
		t.Errorf("empty use = %v", got)
	}
	if got := expandUse("foo::{bar"); len(got) != 1 || got[0] != "foo::{bar" {
		t.Errorf("malformed group fallback = %v", got)
	}
	if _, ok := parsePathAttr("other = 1, pathname = 2"); ok {
		t.Error("non-path attribute accepted")
	}
	for _, attr := range []string{"path", "path = bare", "pathology = \"x\""} {
		if _, ok := parsePathAttr(attr); ok {
			t.Errorf("invalid path attribute accepted: %q", attr)
		}
	}
	if isCfgTest("other") || isCfgTest("cfg test") || isCfgTest("cfg(test") {
		t.Error("invalid cfg recognized")
	}
	if mentionsTest(`cfg(feature = "test")`) || !mentionsTest("cfg(any(test, unix))") {
		t.Error("test token classification failed")
	}
}

func TestParserScannerEdges(t *testing.T) {
	p := &parser{src: "not-an-attribute"}
	if _, _, ok := p.scanAttr(); ok {
		t.Error("non-attribute accepted")
	}
	p = &parser{src: "# nope"}
	if _, _, ok := p.scanAttr(); ok {
		t.Error("attribute without bracket accepted")
	}
	p = &parser{src: "#[unterminated"}
	if _, _, ok := p.scanAttr(); ok {
		t.Error("unterminated attribute accepted")
	}
	p = &parser{src: "#! [inner]"}
	if body, inner, ok := p.scanAttr(); !ok || !inner || body != "inner" {
		t.Errorf("inner attribute = %q %v %v", body, inner, ok)
	}
	p = &parser{src: "  42"}
	if _, ok := p.scanWord(); ok {
		t.Error("numeric word accepted")
	}
	p = &parser{src: "pub(unclosed"}
	if word, ok := p.scanWord(); !ok || word != "pub" {
		t.Fatal("pub not scanned")
	}
	p.skipPubRestrict()
	if p.pos != 3 {
		t.Errorf("unbalanced restriction advanced to %d", p.pos)
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
	} {
		if _, err := ParseFacts("ambiguous.rs", []byte(source)); err == nil {
			t.Errorf("ambiguous item accepted: %s", source)
		}
	}
}

func TestParseFunctionAndExternEdges(t *testing.T) {
	facts := mustParse(t, "edge.rs", "extern crate plain next;\nfn declaration();\nfn unclosed() {\n")
	if len(facts.Externs) != 1 || facts.Externs[0].Name != "plain" {
		t.Errorf("externs = %+v", facts.Externs)
	}
}

func TestUseAndAttributeNestedEdges(t *testing.T) {
	if got := expandUse("{foo, bar}"); strings.Join(got, ",") != "foo,bar" {
		t.Errorf("root use group = %v", got)
	}
	if got := expandUse("foo::{,bar}"); len(got) != 1 || got[0] != "foo::bar" {
		t.Errorf("empty grouped use = %v", got)
	}
	p := &parser{src: "#[nested[a]]"}
	if body, _, ok := p.scanAttr(); !ok || body != "nested[a]" {
		t.Errorf("nested attribute = %q %v", body, ok)
	}
}

func TestParserCoverageClosure(t *testing.T) {
	cleaned := cleanSource("#[foo([bar])]\nmod m;\n")
	if !strings.Contains(cleaned, "mod m") {
		t.Errorf("nested attr bracket blanked module: %q", cleaned)
	}
	facts := mustParse(t, "closure.rs", "#[foo([bar])]\nmod m;\n")
	if len(facts.Modules) != 1 || facts.Modules[0].Name != "m" {
		t.Errorf("modules = %+v", facts.Modules)
	}
	facts = mustParse(t, "closure.rs", "# lone\nmod m;\n")
	if len(facts.Modules) != 1 || facts.Modules[0].Name != "m" {
		t.Errorf("lone hash modules = %+v", facts.Modules)
	}
	facts = mustParse(t, "closure.rs", "extern C foo;\n")
	if len(facts.Externs) != 0 {
		t.Errorf("externs = %+v, want none", facts.Externs)
	}
	if end, ok := scanChar([]byte("'x'"), 0); !ok || end != 3 {
		t.Errorf("scanChar('x') = %d %v", end, ok)
	}
	if _, ok := scanChar([]byte("'\\x"), 0); ok {
		t.Error("unterminated escape accepted as char literal")
	}
	if end := scanString([]byte("bx"), 0); end != 1 {
		t.Errorf("scanString(bx) = %d, want 1", end)
	}
	if !mentionsTest("doc = \"a\\\\b\" test") {
		t.Error("escaped string hid the test token")
	}
}

func TestPrivateScannerBranches(t *testing.T) {
	if end := scanString([]byte(`r#"unterminated`), 0); end != len(`r#"unterminated`) {
		t.Errorf("unterminated raw end = %d", end)
	}
	if end := scanString([]byte("r#bad"), 0); end != 1 {
		t.Errorf("invalid raw prefix end = %d", end)
	}
	if end := scanString([]byte("'a\n"), 0); end != 2 {
		t.Errorf("newline char end = %d", end)
	}
	for _, source := range []string{"'\\u0041'", "'abcdefgh'", "'\\x'"} {
		scanChar([]byte(source), 0)
	}
	if !isTestAttr("test") || isTestAttr("cfg(any(test, unix))") {
		t.Error("isTestAttr classification failed")
	}
	if value, ok := parsePathAttr(`path = "a\\b.rs"`); !ok || value != `a\b.rs` {
		t.Errorf("escaped path = %q %v", value, ok)
	}
	p := &parser{path: "direct.rs", src: " body;"}
	facts := &FileFacts{}
	if err := p.parseUse([]string{"cfg(test)"}, false, facts); err != nil || !facts.HasCfgTest {
		t.Errorf("direct test use = %+v, %v", facts, err)
	}
	p = &parser{path: "direct.rs", src: " crate name;"}
	facts = &FileFacts{}
	if err := p.parseExtern([]string{"cfg(test)"}, false, facts); err != nil || !facts.HasCfgTest {
		t.Errorf("direct test extern = %+v, %v", facts, err)
	}
	p = &parser{path: "direct.rs", src: " declaration();"}
	facts = &FileFacts{}
	if err := p.parseFn([]string{"cfg(test)"}, false, facts); err != nil || !facts.HasTestAttr {
		t.Errorf("direct test fn = %+v, %v", facts, err)
	}
}
