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
	if scripts == nil && common.OpensTag(body, "script") {
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
