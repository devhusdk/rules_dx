package common

import (
	"bytes"

	"golang.org/x/net/html"
)

type ScriptScope int

const (
	TopLevel ScriptScope = iota
	Anywhere
)

var voidElements = map[string]bool{
	"area": true, "base": true, "br": true, "col": true, "embed": true,
	"hr": true, "img": true, "input": true, "link": true, "meta": true,
	"param": true, "source": true, "track": true, "wbr": true,
}

func ExtractScripts(src []byte, scope ScriptScope) [][]byte {
	z := html.NewTokenizer(bytes.NewReader(src))
	var out [][]byte
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
			if name == "script" {
				start = off
				continue
			}
			if scope != TopLevel || voidElements[name] {
				continue
			}
			if end := matchingEnd(src, off, raw); end >= 0 {
				off = end
				z = html.NewTokenizer(bytes.NewReader(src[off:]))
			}
		case html.EndTagToken:
			raw, _ := z.TagName()
			if string(raw) == "script" && start >= 0 {
				out = append(out, src[start:pos])
				start = -1
			}
		}
	}
}

func matchingEnd(src []byte, from int, name []byte) int {
	depth := 1
	i := from
	for {
		lt := bytes.IndexByte(src[i:], '<')
		if lt < 0 {
			return -1
		}
		i += lt
		rest := src[i:]
		switch {
		case bytes.HasPrefix(rest, []byte("<!--")):
			end := bytes.Index(rest[4:], []byte("-->"))
			if end < 0 {
				return -1
			}
			i += 4 + end + 3
		case len(rest) > 1 && rest[1] == '/' && tagNameAt(rest[2:], name):
			depth--
			gt := bytes.IndexByte(rest, '>')
			if gt < 0 {
				return -1
			}
			if depth == 0 {
				return i + gt + 1
			}
			i += gt + 1
		case tagNameAt(rest[1:], name):
			if gt := bytes.IndexByte(rest, '>'); gt < 0 || rest[gt-1] != '/' {
				depth++
			}
			i++
		default:
			i++
		}
	}
}

func tagNameAt(b, name []byte) bool {
	if len(b) < len(name) || !bytes.EqualFold(b[:len(name)], name) {
		return false
	}
	if len(b) == len(name) {
		return true
	}
	switch b[len(name)] {
	case ' ', '\t', '\n', '\r', '\f', '/', '>':
		return true
	}
	return false
}
