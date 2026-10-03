package astro

import (
	"bytes"
	"github.com/ralvik/rules_dx/gazelle/common"
	"sort"
)

func ParseImports(content []byte) []string {
	body := content
	var regions [][]byte
	if fenced, front, rest, ok := splitFence(content); fenced {
		if !ok {
			return nil
		}
		regions = append(regions, front)
		body = rest
	}
	scripts := common.ExtractScripts(body)
	if scripts == nil && opensScript(body) {
		return nil
	}
	regions = append(regions, scripts...)
	if len(regions) == 0 {
		return nil
	}
	set := make(map[string]struct{})
	add := func(spec string) {
		root := common.NormalizeJSSpec(spec)
		if root == "" {
			return
		}
		set[root] = struct{}{}
	}
	for _, region := range regions {
		common.ScanEmbedded(region, add)
	}
	var out []string
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out
}

func splitFence(src []byte) (fenced bool, front, rest []byte, ok bool) {
	var first, after []byte
	if nl := bytes.IndexByte(src, '\n'); nl < 0 {
		first, after = src, nil
	} else {
		first, after = src[:nl], src[nl+1:]
	}
	if string(trimFenceLine(first)) != "---" {
		return false, nil, nil, false
	}
	start := 0
	for {
		rel := bytes.IndexByte(after[start:], '\n')
		var line []byte
		var next int
		if rel < 0 {
			line = after[start:]
			next = len(after)
		} else {
			line = after[start : start+rel]
			next = start + rel + 1
		}
		if string(trimFenceLine(line)) == "---" {
			return true, after[:start], after[next:], true
		}
		if rel < 0 {
			return true, nil, nil, false
		}
		start = next
	}
}

func trimFenceLine(line []byte) []byte {
	return bytes.TrimRight(line, " \t\r")
}

func opensScript(body []byte) bool {
	n := len(body)
	i := 0
	for i < n {
		if i+4 <= n && body[i] == '<' && body[i+1] == '!' && body[i+2] == '-' && body[i+3] == '-' {
			j := i + 4
			end := -1
			for j+2 < n {
				if body[j] == '-' && body[j+1] == '-' && body[j+2] == '>' {
					end = j + 3
					break
				}
				j++
			}
			if end < 0 {
				return false
			}
			i = end
			continue
		}
		if body[i] != '<' {
			i++
			continue
		}
		if i+1 < n && body[i+1] == '/' {
			i += 2
			continue
		}
		name, after, ok := scanTagName(body, i+1)
		if !ok {
			i++
			continue
		}
		if equalFold(name, "script") {
			return true
		}
		i = after
	}
	return false
}

func scanTagName(src []byte, i int) ([]byte, int, bool) {
	n := len(src)
	j := i
	for j < n && isTagNameChar(src[j]) {
		j++
	}
	if j == i {
		return nil, i, false
	}
	return src[i:j], j, true
}

func equalFold(a []byte, b string) bool {
	if len(a) != len(b) {
		return false
	}
	for i := 0; i < len(a); i++ {
		ca := a[i]
		if ca >= 'A' && ca <= 'Z' {
			ca += 'a' - 'A'
		}
		if ca != b[i] {
			return false
		}
	}
	return true
}

func isIdentStart(c byte) bool {
	return c == '_' || c == '$' ||
		(c >= 'a' && c <= 'z') ||
		(c >= 'A' && c <= 'Z')
}

func isTagNameChar(c byte) bool {
	return c == '_' || c == ':' || c == '-' ||
		(c >= 'a' && c <= 'z') ||
		(c >= 'A' && c <= 'Z') ||
		(c >= '0' && c <= '9')
}
