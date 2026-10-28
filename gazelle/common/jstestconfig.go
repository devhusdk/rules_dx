package common

import (
	"path"
	"strings"
)

var jsTestConfigKeys = map[string]bool{
	"setupFiles":          true,
	"setupFilesAfterEnv":  true,
	"globalSetup":         true,
	"globalTeardown":      true,
	"snapshotSerializers": true,
}

// IsJSTestConfig reports whether name is a Jest, Vitest, or Vite config file.
func IsJSTestConfig(name string) bool {
	base := path.Base(name)
	stem := strings.TrimSuffix(base, path.Ext(base))
	if !strings.HasSuffix(stem, ".config") || !HasExt(base, JSModuleExts) {
		return false
	}
	switch strings.TrimSuffix(stem, ".config") {
	case "jest", "vitest", "vite":
		return true
	}
	return false
}

// ScanJSTestConfig reports the literal module specs of setupFiles,
// setupFilesAfterEnv, globalSetup, globalTeardown, and snapshotSerializers.
// A leading <rootDir>/ is read as the config's own directory.
func ScanJSTestConfig(src []byte, add func(string)) {
	n := len(src)
	addSpec := func(spec string) {
		if rest, ok := strings.CutPrefix(spec, "<rootDir>/"); ok {
			spec = "./" + rest
		}
		if spec != "" && !strings.Contains(spec, "<") {
			add(spec)
		}
	}
	i := 0
	for i < n {
		c := src[i]
		if np, ok := skipComment(src, i); ok {
			i = np
			continue
		}
		var key string
		next := i + 1
		switch {
		case c == '\'' || c == '"':
			s, np, ok := parseQuoted(src, i)
			if !ok {
				i = np
				continue
			}
			key, next = s, np
		case c == '`':
			i = skipTemplate(src, i)
			continue
		case c == '/' && isRegexStart(src, i):
			i = skipRegex(src, i)
			continue
		case isIdentStart(c):
			j := i + 1
			for j < n && isIdentChar(src[j]) {
				j++
			}
			if isPrecededByDot(src, i) {
				i = j
				continue
			}
			key, next = string(src[i:j]), j
		}
		i = next
		if !jsTestConfigKeys[key] {
			continue
		}
		p := skipTrivia(src, next)
		if p >= n || src[p] != ':' {
			continue
		}
		p = skipTrivia(src, p+1)
		if p >= n {
			return
		}
		switch src[p] {
		case '\'', '"':
			if s, np, ok := parseQuoted(src, p); ok {
				addSpec(s)
				i = np
			}
		case '[':
			i = scanStringArray(src, p+1, addSpec)
		}
	}
}

// scanStringArray reports the string literal elements of an array starting
// after its '[' and skips any other element.
func scanStringArray(src []byte, p int, add func(string)) int {
	n := len(src)
	depth := 0
	sep := true
	for p < n {
		p = skipTrivia(src, p)
		if p >= n {
			return p
		}
		c := src[p]
		if c == ',' && depth == 0 {
			sep = true
			p++
			continue
		}
		wasSep := sep
		sep = false
		switch c {
		case '\'', '"':
			s, np, ok := parseQuoted(src, p)
			if !ok {
				return np
			}
			if depth == 0 && wasSep {
				q := skipTrivia(src, np)
				if q < n && (src[q] == ',' || src[q] == ']') {
					add(s)
				}
			}
			p = np
		case '`':
			p = skipTemplate(src, p)
		case '[', '(', '{':
			depth++
			p++
		case ')', '}':
			depth--
			p++
		case ']':
			if depth == 0 {
				return p + 1
			}
			depth--
			p++
		default:
			p++
		}
	}
	return p
}
