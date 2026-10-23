package gleam

import (
	"strings"
	"testing"
)

func TestParseImportsKeepsModuleNames(t *testing.T) {
	src := `import gleam/io
import gleam/list.{map, filter}
import app/core as c
//import commented_out
import app/core
  import indented
pub fn f() { "import in_a_string" }
`
	got := strings.Join(ParseImports([]byte(src)), ",")
	if got != "app/core,gleam/io,gleam/list,indented" {
		t.Errorf("imports = %q", got)
	}
}
