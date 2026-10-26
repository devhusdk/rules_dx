package zig

import (
	"strings"
	"testing"
)

func TestParseImportsKeepsModuleNames(t *testing.T) {
	src := `const std = @import("std");
const util = @import("util.zig");
const data = @import("data.zon");
const greet = @import ( "greet" );
// const dead = @import("commented_out");
/// const doc = @import("doc_comment");
const text = "@import(\"in_a_string\")";
const multi =
    \\@import("in_a_multiline_string")
;
const quote = '"';
const json = @import("json_lib");
const again = @import("greet");
`
	got := strings.Join(ParseImports([]byte(src)), ",")
	if got != "greet,json_lib,std" {
		t.Errorf("imports = %q, want greet,json_lib,std", got)
	}
}

func TestParseImportsIgnoresDynamicArguments(t *testing.T) {
	src := "const name = \"x\";\nconst m = @import(name);\nconst n = @import(\"a\" ++ \"b\");\n"
	if got := ParseImports([]byte(src)); len(got) != 0 {
		t.Errorf("imports = %v, want none", got)
	}
}

func TestParseImportsSurvivesUnterminatedText(t *testing.T) {
	for _, src := range []string{
		"const s = \"open", "const s = \"x\\", "@import(", "'",
		"@import(\"greet\n)", "@embedFile(\"a.txt\n)", "@import(\"a.zig\n)", "@import(\"std",
	} {
		if got := ParseImports([]byte(src)); len(got) != 0 {
			t.Errorf("ParseImports(%q) = %v, want none", src, got)
		}
		if got := ParseFileRefs([]byte(src)); len(got) != 0 {
			t.Errorf("ParseFileRefs(%q) = %v, want none", src, got)
		}
		if DefinesMain([]byte(src)) {
			t.Errorf("DefinesMain(%q) = true, want false", src)
		}
	}
}

func TestDefinesMain(t *testing.T) {
	cases := map[string]bool{
		"pub fn main() !void {}\n":                             true,
		"pub fn main(init: std.process.Init) !void {}\n":       true,
		"fn main() void {}\n":                                  false,
		"// pub fn main() void {}\n":                           false,
		"const s = \"pub fn main() void {}\";\n":               false,
		"pub fn mainLoop() void {}\n":                          false,
		"const x = struct {\n    pub fn other() void {}\n};\n": false,
	}
	for src, want := range cases {
		if got := DefinesMain([]byte(src)); got != want {
			t.Errorf("DefinesMain(%q) = %v, want %v", src, got, want)
		}
	}
}

func TestStdlibModules(t *testing.T) {
	for _, name := range []string{"std", "builtin", "root", "bazel_builtin", "c"} {
		if !IsStdLib(name) {
			t.Errorf("%s is not stdlib", name)
		}
	}
	if IsStdLib("greet") {
		t.Error("greet is stdlib")
	}
}

func TestParseFileRefs(t *testing.T) {
	src := `const std = @import("std");
const util = @import("util.zig");
const data = @import("data.zon");
const text = @embedFile("greeting.txt");
const again = @embedFile ( "greeting.txt" );
// const dead = @embedFile("commented_out.txt");
const quoted = "@embedFile(\"in_a_string.txt\")";
const multi =
    \\@embedFile("in_a_multiline_string.txt")
;
const name = "dynamic.txt";
const dynamic = @embedFile(name);
const empty = @embedFile("");
`
	got := strings.Join(ParseFileRefs([]byte(src)), ",")
	if got != "data.zon,greeting.txt,util.zig" {
		t.Errorf("file refs = %q, want data.zon,greeting.txt,util.zig", got)
	}
}
