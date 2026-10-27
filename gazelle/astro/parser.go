package astro

import (
	"bytes"
	"github.com/devhusdk/rules_dx/gazelle/common"
)

func ParseImports(content []byte, dir string) []common.ImportRef {
	body := content
	var regions [][]byte
	if fenced, front, rest, ok := splitFence(content); fenced {
		if !ok {
			return nil
		}
		regions = append(regions, front)
		body = rest
	}
	scripts := common.ExtractScripts(body, common.Anywhere)
	regions = append(regions, scripts...)
	set := common.RefSet{Dir: dir}
	for _, region := range regions {
		common.ScanEmbedded(region, set.Add)
	}
	return set.Refs()
}

// splitFence only frames the frontmatter; ScanEmbedded reads its imports.
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
