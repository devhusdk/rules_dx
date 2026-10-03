package mdx

import (
	"github.com/ralvik/rules_dx/gazelle/common"
	"strings"
)

func ParseImports(content []byte) []string {
	var set common.SpecSet
	for _, region := range extractESMRegions(content) {
		common.ScanEmbedded(region, set.Add)
	}
	return set.Roots()
}

func extractESMRegions(src []byte) [][]byte {
	lines := splitLines(src)
	var out [][]byte
	var chunk []string
	depth := 0
	boundary := true
	prevText := false
	inFence := false
	fenceChar := byte(0)
	fenceLen := 0
	flush := func() {
		if len(chunk) > 0 {
			out = append(out, []byte(strings.Join(chunk, "\n")+"\n"))
			chunk = nil
		}
		depth = 0
	}
	for _, raw := range lines {
		line := raw
		if !inFence {
			if idx := strings.Index(line, "<!--"); idx >= 0 {
				if end := strings.Index(line[idx+4:], "-->"); end >= 0 {
					line = line[:idx] + line[idx+4+end+3:]
				} else {
					return nil
				}
			}
		}
		if !inFence {
			if ch, ln, ok := parseFenceOpen(line); ok {
				flush()
				inFence = true
				fenceChar = ch
				fenceLen = ln
				boundary = false
				prevText = false
				continue
			}
		} else {
			if isFenceClose(line, fenceChar, fenceLen) {
				inFence = false
				boundary = true
				prevText = false
				continue
			}
			continue
		}
		if strings.TrimSpace(line) == "" {
			if depth == 0 {
				flush()
			} else {
				chunk = append(chunk, line)
			}
			boundary = true
			prevText = false
			continue
		}
		if isESMOpener(line) {
			if len(chunk) > 0 || boundary {
				chunk = append(chunk, line)
				depth += braceDelta(line)
				boundary = true
				prevText = false
				continue
			}
			boundary = false
			prevText = true
			continue
		}
		if len(chunk) > 0 && depth > 0 && isESMContinuation(line) {
			chunk = append(chunk, line)
			depth += braceDelta(line)
			boundary = false
			prevText = false
			continue
		}
		flush()
		boundary = isBoundaryLine(line, prevText)
		prevText = !boundary
	}
	flush()
	if len(out) == 0 {
		return nil
	}
	return out
}

func splitLines(src []byte) []string {
	if len(src) == 0 {
		return nil
	}
	raw := strings.Split(string(src), "\n")
	if raw[len(raw)-1] == "" {
		raw = raw[:len(raw)-1]
	}
	for i := range raw {
		raw[i] = strings.TrimSuffix(raw[i], "\r")
	}
	return raw
}

func isESMOpener(line string) bool {
	if strings.HasPrefix(line, "import") {
		if len(line) == 6 {
			return false
		}
		switch line[6] {
		case ' ', '\t', '"', '\'', '(', '{', '*':
			return true
		}
		return false
	}
	if strings.HasPrefix(line, "export") {
		if len(line) == 6 {
			return false
		}
		switch line[6] {
		case ' ', '\t', '{', '*':
			return true
		}
		return false
	}
	return false
}

func isESMContinuation(line string) bool {
	trimmed := strings.TrimLeft(line, " \t")
	return strings.HasPrefix(trimmed, "}") || strings.HasPrefix(trimmed, "from ") || strings.HasPrefix(trimmed, "from\t")
}

func isBoundaryLine(line string, prevText bool) bool {
	if isATXHeading(line) || isThematicBreak(line) {
		return true
	}
	if prevText && isSetextUnderline(line) {
		return true
	}
	return false
}

func isATXHeading(line string) bool {
	i := 0
	for i < len(line) && line[i] == ' ' && i < 4 {
		i++
	}
	if i >= 4 {
		return false
	}
	hashes := 0
	for i < len(line) && line[i] == '#' {
		hashes++
		i++
	}
	if hashes == 0 || hashes > 6 {
		return false
	}
	if i >= len(line) {
		return true
	}
	return line[i] == ' ' || line[i] == '\t'
}

func isThematicBreak(line string) bool {
	stripped := strings.ReplaceAll(strings.ReplaceAll(line, " ", ""), "\t", "")
	if len(stripped) < 3 {
		return false
	}
	mark := stripped[0]
	if mark != '-' && mark != '*' && mark != '_' {
		return false
	}
	for i := 1; i < len(stripped); i++ {
		if stripped[i] != mark {
			return false
		}
	}
	return true
}

func isSetextUnderline(line string) bool {
	stripped := strings.ReplaceAll(strings.ReplaceAll(line, " ", ""), "\t", "")
	if len(stripped) < 1 {
		return false
	}
	for i := 0; i < len(stripped); i++ {
		if stripped[i] != '=' {
			return false
		}
	}
	return true
}

func parseFenceOpen(line string) (byte, int, bool) {
	i := 0
	for i < len(line) && line[i] == ' ' {
		i++
	}
	if i > 3 {
		return 0, 0, false
	}
	if i >= len(line) || (line[i] != '`' && line[i] != '~') {
		return 0, 0, false
	}
	ch := line[i]
	n := 0
	for i < len(line) && line[i] == ch {
		n++
		i++
	}
	if n < 3 {
		return 0, 0, false
	}
	return ch, n, true
}

func isFenceClose(line string, ch byte, ln int) bool {
	i := 0
	for i < len(line) && line[i] == ' ' {
		i++
	}
	if i > 3 {
		return false
	}
	n := 0
	for i < len(line) && line[i] == ch {
		n++
		i++
	}
	if n < ln {
		return false
	}
	for i < len(line) {
		if line[i] != ' ' && line[i] != '\t' {
			return false
		}
		i++
	}
	return true
}

func braceDelta(line string) int {
	depth := 0
	i := 0
	for i < len(line) {
		c := line[i]
		if c == '/' && i+1 < len(line) && line[i+1] == '/' {
			break
		}
		if c == '/' && i+1 < len(line) && line[i+1] == '*' {
			j := strings.Index(line[i+2:], "*/")
			if j < 0 {
				break
			}
			i += j + 4
			continue
		}
		if c == '\'' || c == '"' {
			j := i + 1
			for j < len(line) {
				if line[j] == '\\' {
					j += 2
					continue
				}
				if line[j] == c {
					break
				}
				j++
			}
			i = j + 1
			continue
		}
		if c == '{' {
			depth++
		} else if c == '}' {
			depth--
		}
		i++
	}
	return depth
}
