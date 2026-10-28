package dart

import (
	"reflect"
	"sort"
	"strings"
	"testing"
)

func keys(m map[string]bool) []string {
	var out []string
	for k := range m {
		out = append(out, k)
	}
	sort.Strings(out)
	return out
}

func TestParsePubspec(t *testing.T) {
	p, err := ParsePubspec([]byte(`name: demo # comment
version: 1.0.0
resolution: workspace
environment:
  sdk: ^3.5.0
dependencies:
  collection: ^1.19.0
  local:
    path: ../local
  flutter: {sdk: flutter}
dev_dependencies:
  test: ^1.25.0
workspace:
  - packages/a
  - "packages/b"
`))
	if err != nil {
		t.Fatal(err)
	}
	if p.Name != "demo" || p.Resolution != "workspace" {
		t.Errorf("got %+v", p)
	}
	if got := keys(p.Deps); !reflect.DeepEqual(got, []string{"collection", "flutter", "local"}) {
		t.Errorf("deps = %v", got)
	}
	if got := keys(p.DevDeps); !reflect.DeepEqual(got, []string{"test"}) {
		t.Errorf("dev deps = %v", got)
	}
	if !reflect.DeepEqual(p.Workspace, []string{"packages/a", "packages/b"}) {
		t.Errorf("workspace = %v", p.Workspace)
	}
}

func TestParsePubspecRejects(t *testing.T) {
	for name, src := range map[string]string{
		"no name":  "version: 1.0.0\n",
		"bad name": "name: Bad-Name\n",
		"reserved": "name: class\n",
		"leading digit": "name: 1abc\n",
		"scalar deps":   "name: a\ndependencies: foo\n",
	} {
		_, err := ParsePubspec([]byte(src))
		if err == nil {
			t.Errorf("%s: want an error", name)
		}
	}
	if _, err := ParsePubspec([]byte("name: class\n")); err == nil || !strings.Contains(err.Error(), "reserved") {
		t.Errorf("reserved word error = %v", err)
	}
}
