package erlang

import "regexp"

// AppSrc holds the literal facts dx generate reads from src/<app>.app.src.
type AppSrc struct {
	Name        string
	Version     string
	Description string
	Module      string
}

var (
	commentPattern = regexp.MustCompile(`(?m)%.*$`)
	appNamePattern = regexp.MustCompile(`\{\s*application\s*,\s*'?([a-z][A-Za-z0-9_@]*)'?`)
	descPattern    = regexp.MustCompile(`\{\s*description\s*,\s*"((?:[^"\\]|\\.)*)"`)
	vsnPattern     = regexp.MustCompile(`\{\s*vsn\s*,\s*"([^"\\]*)"`)
	modPattern     = regexp.MustCompile(`\{\s*mod\s*,\s*\{\s*'?([a-z][A-Za-z0-9_@]*)'?`)
)

// ParseAppSrc reads the application name, vsn, description and mod from an
// .app.src. Comments are dropped first; non-literal values stay empty.
func ParseAppSrc(content []byte) AppSrc {
	text := commentPattern.ReplaceAllString(string(content), "")
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
