package erlang

import (
	"regexp"
	"strings"
)

// AppSrc holds the literal facts dx generate reads from src/<app>.app.src.
type AppSrc struct {
	Name        string
	Version     string
	Description string
	Module      string
}

var (
	appNamePattern = regexp.MustCompile(`\{\s*application\s*,\s*'?([a-z][A-Za-z0-9_@]*)'?`)
	descPattern    = regexp.MustCompile(`\{\s*description\s*,\s*"((?:[^"\\]|\\.)*)"`)
	vsnPattern     = regexp.MustCompile(`\{\s*vsn\s*,\s*"([^"\\]*)"`)
	modPattern     = regexp.MustCompile(`\{\s*mod\s*,\s*\{\s*'?([a-z][A-Za-z0-9_@]*)'?`)
)

// ParseAppSrc reads the application name, vsn, description and mod from an
// .app.src. Comments are dropped first; non-literal values stay empty.
func ParseAppSrc(content []byte) AppSrc {
	text := stripComments(string(content))
	first := func(re *regexp.Regexp) string {
		if m := re.FindStringSubmatch(text); m != nil {
			return m[1]
		}
		return ""
	}
	return AppSrc{
		Name:        first(appNamePattern),
		Version:     first(vsnPattern),
		Description: first(descPattern),
		Module:      first(modPattern),
	}
}

func stripComments(s string) string {
	var b strings.Builder
	for i := 0; i < len(s); i++ {
		switch c := s[i]; c {
		case '"', '\'':
			j := i + 1
			for j < len(s) && s[j] != c {
				if s[j] == '\\' {
					j++
				}
				j++
			}
			j = min(j+1, len(s))
			b.WriteString(s[i:j])
			i = j - 1
		case '$':
			j := min(i+2, len(s))
			if i+1 < len(s) && s[i+1] == '\\' {
				j = min(i+3, len(s))
			}
			b.WriteString(s[i:j])
			i = j - 1
		case '%':
			for i < len(s) && s[i] != '\n' {
				i++
			}
			if i < len(s) {
				b.WriteByte('\n')
			}
		default:
			b.WriteByte(c)
		}
	}
	return b.String()
}
