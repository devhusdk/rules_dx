package dart

import (
	"fmt"
	"strings"
	"sync"

	"github.com/devhusdk/rules_dx/gazelle/common/treesitter"
	ts "github.com/odvcencio/gotreesitter"
	"github.com/odvcencio/gotreesitter/grammars/dart"
)

var grammar = sync.OnceValue(func() *ts.Language { return dart.Language() })

// Directive is one import, export or part URI of a Dart file.
type Directive struct {
	Kind string // import, export or part
	URI  string
}

var directiveKeywords = map[string]bool{"import": true, "export": true, "part": true, "library": true}

// ParseDirectives returns every import, export and part URI of content. A
// conditional import yields all of its URIs. A syntax error before the first
// declaration is an error, because the directives cannot be trusted.
func ParseDirectives(content []byte) ([]Directive, error) {
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	if tree.Root == nil {
		return nil, fmt.Errorf("cannot parse Dart source")
	}
	var out []Directive
	for _, c := range tree.Root.Children() {
		typ := tree.Type(c)
		if strings.Contains(typ, "comment") || strings.Contains(typ, "script") {
			continue
		}
		if typ == "ERROR" || c.HasError() {
			return nil, fmt.Errorf("syntax error in the directives at byte %d", c.StartByte())
		}
		kind := firstWord(stripMetadata(tree.Text(c)))
		if !directiveKeywords[kind] {
			// First declaration: the directive region is over.
			return out, nil
		}
		isPartOf := false
		if kind == "part" {
			isPartOf = strings.HasPrefix(strings.TrimSpace(strings.TrimPrefix(strings.TrimSpace(stripMetadata(tree.Text(c))), "part")), "of")
		}
		if kind == "library" || isPartOf {
			continue
		}
		uris, err := directiveURIs(tree, c)
		if err != nil {
			return nil, err
		}
		if len(uris) == 0 {
			return nil, fmt.Errorf("%s directive at byte %d has no URI", kind, c.StartByte())
		}
		for _, u := range uris {
			out = append(out, Directive{Kind: kind, URI: u})
		}
	}
	return out, nil
}

// stripMetadata removes leading annotations (@name, @a.b, with an optional
// balanced argument list), whitespace and comments.
func stripMetadata(s string) string {
	for {
		s = strings.TrimSpace(s)
		switch {
		case strings.HasPrefix(s, "//"):
			i := strings.IndexByte(s, '\n')
			if i < 0 {
				return ""
			}
			s = s[i+1:]
		case strings.HasPrefix(s, "/*"):
			i := strings.Index(s, "*/")
			if i < 0 {
				return ""
			}
			s = s[i+2:]
		case strings.HasPrefix(s, "@"):
			i := 1
			for i < len(s) && (s[i] == '_' || s[i] == '$' || s[i] == '.' || s[i] >= '0' && s[i] <= '9' || s[i] >= 'a' && s[i] <= 'z' || s[i] >= 'A' && s[i] <= 'Z') {
				i++
			}
			s = strings.TrimLeft(s[i:], " \t\r\n")
			if strings.HasPrefix(s, "(") {
				depth, j := 0, 0
				var quote byte
				for ; j < len(s); j++ {
					ch := s[j]
					if quote != 0 {
						if ch == '\\' {
							j++
						} else if ch == quote {
							quote = 0
						}
						continue
					}
					if ch == '\'' || ch == '"' {
						quote = ch
					} else if ch == '(' {
						depth++
					} else if ch == ')' {
						depth--
						if depth == 0 {
							j++
							break
						}
					}
				}
				s = s[min(j, len(s)):]
			}
		default:
			return s
		}
	}
}

func firstWord(s string) string {
	s = strings.TrimSpace(s)
	end := 0
	for end < len(s) && (s[end] >= 'a' && s[end] <= 'z') {
		end++
	}
	return s[:end]
}

// directiveURIs collects the URI string literals of a directive, leaving out
// the string compared in a conditional import's test.
func directiveURIs(tree *treesitter.Tree, n *treesitter.Node) ([]string, error) {
	var out []string
	var err error
	var walk func(n *treesitter.Node, afterEq bool)
	walk = func(n *treesitter.Node, afterEq bool) {
		prevEq := false
		for _, c := range n.Children() {
			typ := tree.Type(c)
			switch {
			case strings.Contains(typ, "comment"), typ == "annotation":
			case typ == "string_literal":
				if !prevEq {
					u, e := decodeString(tree.Text(c))
					if e != nil && err == nil {
						err = e
					}
					out = append(out, u)
				}
			default:
				walk(c, false)
			}
			if c.ChildCount() == 0 {
				prevEq = tree.Text(c) == "=="
			} else {
				prevEq = false
			}
		}
	}
	walk(n, false)
	return out, err
}

func decodeString(lit string) (string, error) {
	s := lit
	raw := false
	if strings.HasPrefix(s, "r") {
		raw = true
		s = s[1:]
	}
	for _, q := range []string{`'''`, `"""`, `'`, `"`} {
		if strings.HasPrefix(s, q) && strings.HasSuffix(s, q) && len(s) >= 2*len(q) {
			s = s[len(q) : len(s)-len(q)]
			if !raw && strings.ContainsAny(s, "$\\") {
				return "", fmt.Errorf("URI %s uses an interpolation or escape; write a plain string", lit)
			}
			if s == "" {
				return "", fmt.Errorf("empty URI")
			}
			return s, nil
		}
	}
	return "", fmt.Errorf("cannot read URI %s", lit)
}
