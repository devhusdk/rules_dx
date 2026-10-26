package mdx

import (
	"bytes"

	"github.com/devhusdk/rules_dx/gazelle/common"
	"github.com/yuin/goldmark/ast"
	"github.com/yuin/goldmark/parser"
	"github.com/yuin/goldmark/text"
	"github.com/yuin/goldmark/util"
)

var kindESM = ast.NewNodeKind("MDXjsESM")

type esmNode struct {
	ast.BaseBlock
}

func (n *esmNode) Kind() ast.NodeKind { return kindESM }

func (n *esmNode) IsRaw() bool { return true }

func (n *esmNode) Dump(source []byte, level int) { ast.DumpHelper(n, source, level, nil, nil) }

type esmBlockParser struct{}

func (esmBlockParser) Trigger() []byte { return []byte{'i', 'e'} }

func (esmBlockParser) Open(parent ast.Node, reader text.Reader, pc parser.Context) (ast.Node, parser.State) {
	if parent.Kind() != ast.KindDocument {
		return nil, parser.NoChildren
	}
	line, seg := reader.PeekLine()
	src := reader.Source()
	if seg.Padding != 0 || (seg.Start > 0 && src[seg.Start-1] != '\n') {
		return nil, parser.NoChildren
	}
	if !bytes.HasPrefix(line, []byte("import ")) && !bytes.HasPrefix(line, []byte("export ")) {
		return nil, parser.NoChildren
	}
	node := &esmNode{}
	node.Lines().Append(seg)
	reader.AdvanceToEOL()
	return node, parser.NoChildren
}

func (esmBlockParser) Continue(node ast.Node, reader text.Reader, pc parser.Context) parser.State {
	line, seg := reader.PeekLine()
	if util.IsBlank(line) && !common.IncompleteJS(esmRegion(node, reader.Source())) {
		return parser.Close
	}
	node.Lines().Append(seg)
	reader.AdvanceToEOL()
	return parser.Continue | parser.NoChildren
}

func (esmBlockParser) Close(node ast.Node, reader text.Reader, pc parser.Context) {}

func (esmBlockParser) CanInterruptParagraph() bool { return false }

func (esmBlockParser) CanAcceptIndentedLine() bool { return false }

func esmRegion(node ast.Node, src []byte) []byte {
	lines := node.Lines()
	return src[lines.At(0).Start:lines.At(lines.Len()-1).Stop]
}

var mdxParser = newMDXParser()

func newMDXParser() parser.Parser {
	return parser.NewParser(parser.WithBlockParsers(
		util.Prioritized(esmBlockParser{}, 50),
		util.Prioritized(parser.NewSetextHeadingParser(), 100),
		util.Prioritized(parser.NewThematicBreakParser(), 200),
		util.Prioritized(parser.NewListParser(), 300),
		util.Prioritized(parser.NewListItemParser(), 400),
		util.Prioritized(parser.NewATXHeadingParser(), 600),
		util.Prioritized(parser.NewFencedCodeBlockParser(), 700),
		util.Prioritized(parser.NewBlockquoteParser(), 800),
		util.Prioritized(parser.NewParagraphParser(), 1000),
	))
}

func extractESMRegions(src []byte) [][]byte {
	doc := mdxParser.Parse(text.NewReader(src))
	var out [][]byte
	broken := false
	_ = ast.Walk(doc, func(n ast.Node, entering bool) (ast.WalkStatus, error) {
		if !entering {
			return ast.WalkContinue, nil
		}
		switch n.Kind() {
		case ast.KindFencedCodeBlock, ast.KindCodeBlock:
			return ast.WalkSkipChildren, nil
		case kindESM:
			out = append(out, esmRegion(n, src))
		}
		if n.Type() == ast.TypeBlock {
			lines := n.Lines()
			for i := 0; i < lines.Len(); i++ {
				seg := lines.At(i)
				if unclosedComment(seg.Value(src)) {
					broken = true
					return ast.WalkStop, nil
				}
			}
		}
		return ast.WalkContinue, nil
	})
	if broken || len(out) == 0 {
		return nil
	}
	return out
}

func unclosedComment(line []byte) bool {
	i := bytes.Index(line, []byte("<!--"))
	return i >= 0 && !bytes.Contains(line[i+4:], []byte("-->"))
}
