package dart

import (
	"fmt"
	"regexp"
	"strings"
)

// Pubspec is the part of a pubspec.yaml that generation reads.
type Pubspec struct {
	Name         string
	Deps         map[string]bool
	DevDeps      map[string]bool
	Workspace    []string
	Resolution   string
	hasWorkspace bool
}

var packageNameRE = regexp.MustCompile(`^[a-z_][a-z0-9_]*$`)

var reservedWords = map[string]bool{}

func init() {
	for _, w := range strings.Fields(`abstract as assert async await break case catch class const continue covariant default deferred do dynamic else enum export extends extension external factory false final finally for function get hide if implements import in interface is late library mixin new null of on operator part required rethrow return sealed set show static super switch sync this throw true try typedef var void while with yield`) {
		reservedWords[w] = true
	}
}

// ValidPackageName reports whether name is a legal pub package name.
func ValidPackageName(name string) error {
	switch {
	case !packageNameRE.MatchString(name):
		return fmt.Errorf("%q is not a valid pub package name; use lowercase letters, digits and underscores, not starting with a digit", name)
	case reservedWords[name]:
		return fmt.Errorf("%q is a Dart reserved word; rename the package", name)
	}
	return nil
}

type yamlLine struct {
	indent int
	text   string
	num    int
}

func stripComment(s string) string {
	quote := byte(0)
	for i := 0; i < len(s); i++ {
		c := s[i]
		switch {
		case quote != 0:
			if c == quote {
				quote = 0
			}
		case c == '"' || c == '\'':
			quote = c
		case c == '#' && (i == 0 || s[i-1] == ' ' || s[i-1] == '\t'):
			return s[:i]
		}
	}
	return s
}

func yamlLines(content []byte) ([]yamlLine, error) {
	var out []yamlLine
	for i, raw := range strings.Split(string(content), "\n") {
		raw = strings.TrimRight(stripComment(raw), " \t\r")
		if strings.TrimSpace(raw) == "" || raw == "---" {
			continue
		}
		if strings.Contains(raw, "\t") && strings.TrimLeft(raw, " ") != strings.TrimLeft(raw, " \t") {
			return nil, fmt.Errorf("line %d: tab indentation", i+1)
		}
		text := strings.TrimLeft(raw, " ")
		out = append(out, yamlLine{indent: len(raw) - len(text), text: text, num: i + 1})
	}
	return out, nil
}

func unquote(s string) string {
	s = strings.TrimSpace(s)
	if len(s) >= 2 && (s[0] == '"' || s[0] == '\'') && s[len(s)-1] == s[0] {
		return s[1 : len(s)-1]
	}
	return s
}

// splitKey cuts "key: value" into its parts.
func splitKey(text string) (key, value string, ok bool) {
	i := strings.Index(text, ":")
	for i >= 0 && i+1 < len(text) && text[i+1] != ' ' {
		next := strings.Index(text[i+1:], ":")
		if next < 0 {
			return "", "", false
		}
		i += 1 + next
	}
	if i < 0 {
		return "", "", false
	}
	return unquote(text[:i]), strings.TrimSpace(text[i+1:]), true
}

// ParsePubspec reads the fields of a pubspec.yaml that generation needs.
func ParsePubspec(content []byte) (*Pubspec, error) {
	lines, err := yamlLines(content)
	if err != nil {
		return nil, err
	}
	p := &Pubspec{Deps: map[string]bool{}, DevDeps: map[string]bool{}}
	for i := 0; i < len(lines); i++ {
		l := lines[i]
		if l.indent != 0 {
			continue
		}
		key, value, ok := splitKey(l.text)
		if !ok {
			return nil, fmt.Errorf("line %d: expected a top-level key", l.num)
		}
		// Child lines are those indented deeper than the key.
		j := i + 1
		for j < len(lines) && lines[j].indent > 0 {
			j++
		}
		children := lines[i+1 : j]
		switch key {
		case "name":
			p.Name = unquote(value)
		case "resolution":
			p.Resolution = unquote(value)
		case "dependencies", "dev_dependencies":
			target := p.Deps
			if key == "dev_dependencies" {
				target = p.DevDeps
			}
			if value != "" && value != "{}" {
				return nil, fmt.Errorf("line %d: %s must be a block mapping", l.num, key)
			}
			indent := -1
			for _, c := range children {
				if indent < 0 {
					indent = c.indent
				}
				if c.indent != indent {
					continue
				}
				name, _, ok := splitKey(c.text)
				if !ok {
					return nil, fmt.Errorf("line %d: expected a package name in %s", c.num, key)
				}
				target[name] = true
			}
		case "workspace":
			p.hasWorkspace = true
			if strings.HasPrefix(value, "[") && strings.HasSuffix(value, "]") {
				for _, item := range strings.Split(value[1:len(value)-1], ",") {
					if item = unquote(item); item != "" {
						p.Workspace = append(p.Workspace, item)
					}
				}
			} else if value != "" {
				return nil, fmt.Errorf("line %d: workspace must be a list", l.num)
			}
			for _, c := range children {
				if !strings.HasPrefix(c.text, "- ") {
					return nil, fmt.Errorf("line %d: workspace must be a list of paths", c.num)
				}
				p.Workspace = append(p.Workspace, unquote(c.text[2:]))
			}
		}
		i = j - 1
	}
	if p.Name == "" {
		return nil, fmt.Errorf("pubspec.yaml has no name")
	}
	if err := ValidPackageName(p.Name); err != nil {
		return nil, err
	}
	return p, nil
}
