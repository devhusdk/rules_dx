package common

import (
	"bytes"

	"golang.org/x/net/html"
)

// ScriptScope selects which <script> elements ExtractScripts returns.
type ScriptScope int

const (
	// TopLevel keeps only scripts that are root blocks, as Vue and Svelte do.
	TopLevel ScriptScope = iota
	// Anywhere keeps scripts at any depth, as Astro does.
	Anywhere
)

var voidElements = map[string]bool{
	"area": true, "base": true, "br": true, "col": true, "embed": true,
	"hr": true, "img": true, "input": true, "link": true, "meta": true,
	"param": true, "source": true, "track": true, "wbr": true,
}

// ExtractScripts returns the bodies of complete <script> elements as
// sub-slices of src. A script with no end tag before EOF is dropped.
func ExtractScripts(src []byte, scope ScriptScope) [][]byte {
	z := html.NewTokenizer(bytes.NewReader(src))
	var out [][]byte
	var stack []string
	off := 0
	start := -1
	for {
		tt := z.Next()
		pos := off
		off += len(z.Raw())
		switch tt {
		case html.ErrorToken:
			if len(out) == 0 {
				return nil
			}
			return out
		case html.SelfClosingTagToken:
			z.NextIsNotRawText()
		case html.StartTagToken:
			raw, _ := z.TagName()
			name := string(raw)
			if name == "script" && (scope == Anywhere || len(stack) == 0) {
				start = off
			}
			if !voidElements[name] {
				stack = append(stack, name)
			}
		case html.EndTagToken:
			raw, _ := z.TagName()
			name := string(raw)
			if name == "script" && start >= 0 {
				out = append(out, src[start:pos])
				start = -1
			}
			for i := len(stack) - 1; i >= 0; i-- {
				if stack[i] == name {
					stack = stack[:i]
					break
				}
			}
		}
	}
}
