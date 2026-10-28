package common

import (
	"reflect"
	"testing"
)

func TestIsJSTestConfig(t *testing.T) {
	for _, name := range []string{"jest.config.js", "jest.config.ts", "jest.config.cjs", "vitest.config.mts", "vite.config.ts", "pkg/jest.config.mjs"} {
		if !IsJSTestConfig(name) {
			t.Errorf("IsJSTestConfig(%q) = false, want true", name)
		}
	}
	for _, name := range []string{"jest.config.json", "jest.setup.js", "config.js", "webpack.config.js", "my.jest.config.js", "jest.js"} {
		if IsJSTestConfig(name) {
			t.Errorf("IsJSTestConfig(%q) = true, want false", name)
		}
	}
}

func TestScanJSTestConfig(t *testing.T) {
	cases := []struct {
		name   string
		source string
		want   []string
	}{
		{"empty", "", nil},
		{"jestArray", "module.exports = { setupFiles: ['./a.js', \"./b\"] };\n", []string{"./a.js", "./b"}},
		{"afterEnv", "export default { setupFilesAfterEnv: ['@testing-library/jest-dom'] };\n", []string{"@testing-library/jest-dom"}},
		{"globalString", "export default { globalSetup: './gs.ts', globalTeardown: \"./gt.ts\" };\n", []string{"./gs.ts", "./gt.ts"}},
		{"quotedKey", "{ \"setupFiles\": [\"./a\"] }\n", []string{"./a"}},
		{"rootDir", "{ setupFiles: ['<rootDir>/setup/a.js'] }\n", []string{"./setup/a.js"}},
		{"otherToken", "{ setupFiles: ['<rootDir>/../<x>.js'] }\n", nil},
		{"vitestNested", "export default defineConfig({ test: { setupFiles: ['./vs.ts'], globalSetup: ['./g.ts'] } });\n", []string{"./vs.ts", "./g.ts"}},
		{"trailingComma", "{ setupFiles: [\n  './a', // first\n  './b',\n] }\n", []string{"./a", "./b"}},
		{"mixed", "{ setupFiles: ['./a', path.join(dir, './x'), x + './y', './y' + x, `./t`, ...rest, './b'] }\n", []string{"./a", "./b"}},
		{"dynamic", "{ setupFiles: files, globalSetup: resolve('./g') }\n", nil},
		{"memberKey", "config.setupFiles = ['./a'];\n", nil},
		{"memberLookup", "x = { a: cfg.setupFiles };\n", nil},
		{"serializers", "{ snapshotSerializers: ['enzyme-to-json/serializer'] }\n", []string{"enzyme-to-json/serializer"}},
		{"otherKeys", "{ testMatch: ['./a'], reporters: ['./r'] }\n", nil},
		{"inString", "const s = 'setupFiles: [\"./a\"]';\n", nil},
		{"inComment", "// setupFiles: ['./a']\n/* globalSetup: './g' */\n", nil},
		{"empty string", "{ globalSetup: '' }\n", nil},
	}
	for _, tc := range cases {
		var got []string
		ScanJSTestConfig([]byte(tc.source), func(s string) { got = append(got, s) })
		if !reflect.DeepEqual(got, tc.want) {
			t.Errorf("%s: ScanJSTestConfig = %q, want %q", tc.name, got, tc.want)
		}
	}
}

func TestParseSourceImportRefs(t *testing.T) {
	src := []byte("import { defineConfig } from 'vitest/config';\nexport default defineConfig({ test: { setupFiles: ['./setup.ts'] } });\n")
	want := []ImportRef{
		{Spec: "./setup.ts", Root: "pkg/setup", ModuleExt: ".ts", Relative: true},
		{Spec: "vitest/config", Root: "vitest/config"},
	}
	if got := ParseSourceImportRefs("vitest.config.ts", src, "pkg"); !reflect.DeepEqual(got, want) {
		t.Errorf("ParseSourceImportRefs = %+v, want %+v", got, want)
	}
	if got := ParseSourceImportRefs("app.ts", src, "pkg"); !reflect.DeepEqual(got, want[1:]) {
		t.Errorf("ParseSourceImportRefs(app.ts) = %+v, want %+v", got, want[1:])
	}
}
