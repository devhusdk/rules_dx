package lua

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
		{"parens", "local a = require(\"a.b\")\n", []string{"a.b"}},
		{"single quotes bare", "local c = require 'c'\n", []string{"c"}},
		{"long bracket arg", "local d = require[[d.e]]\n", []string{"d.e"}},
		{"const attribute", "local x <const> = require(\"k\")\n", []string{"k"}},
		{"line comment", "-- require(\"c\")\n", nil},
		{"block comment", "--[[ require(\"c\") ]]\nlocal a = require(\"real\")\n", []string{"real"}},
		{"long string", "local s = [==[ require(\"s\") ]==]\n", nil},
		{"field require", "local a = foo.require(\"x\")\n", nil},
		{"method require", "local a = obj:require(\"x\")\n", nil},
		{"pcall guard", "local ok = pcall(require, \"x\")\npcall(function() require(\"y\") end)\n", nil},
		{"concat arg", "local a = require(\"q\" .. v)\n", nil},
		{"variable arg", "local a = require(name)\n", nil},
		{"escape", "local a = require(\"a\\x2eb\")\n", nil},
		{"path name", "local a = require(\"a/b\")\n", nil},
		{"duplicates", "require(\"z\")\nrequire(\"a\")\nrequire(\"z\")\n", []string{"a", "z"}},
		{"nested in function", "local function f() return require(\"lazy\") end\n", []string{"lazy"}},
		{"truncated", "local a = require(\"a\")\nlocal function f(\n", []string{"a"}},
	}
	for _, tc := range cases {
		if got := ParseImports([]byte(tc.src)); !reflect.DeepEqual(got, tc.want) && (len(got) != 0 || len(tc.want) != 0) {
			t.Errorf("%s: ParseImports = %v, want %v", tc.name, got, tc.want)
		}
	}
}
