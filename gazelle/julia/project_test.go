package julia

import (
	"strings"
	"testing"
)

const (
	testUUID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
	testStd  = "8dfed614-e22c-5e08-85e1-65c5234f0b40"
)

func TestParseProject(t *testing.T) {
	valid := "name = \"Demo\"\nuuid = \"" + testUUID + "\"\n[deps]\nFoo = \"" + testUUID + "\"\n[extras]\nTest = \"" + testStd + "\"\n[targets]\ntest = [\"Test\"]\n"
	cases := []struct {
		name    string
		content string
		want    string
		errPart string
	}{
		{"valid", valid, "Demo", ""},
		{"bom", "\xef\xbb\xbf" + valid, "Demo", ""},
		{"crlf", strings.ReplaceAll(valid, "\n", "\r\n"), "Demo", ""},
		{"environment", "[deps]\nFoo = \"" + testUUID + "\"\n", "", ""},
		{"bad identifier", "name = \"1Demo\"\n", "", `"1Demo" is not a Julia identifier`},
		{"invalid toml", "name = \n", "", "invalid TOML"},
		{"bad uuid", "name = \"Demo\"\n[deps]\nFoo = \"nope\"\n", "", "[deps] Foo must be a UUID"},
		{"non-string uuid", "name = \"Demo\"\n[extras]\nFoo = 3\n", "", "[extras] Foo must be a UUID"},
		{"absent test extra", "name = \"Demo\"\n[targets]\ntest = [\"Test\"]\n", "", `"Test", which is not in [extras]`},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			p, err := ParseProject("pkg/Project.toml", []byte(tc.content))
			if tc.errPart != "" {
				if err == nil || !strings.Contains(err.Error(), tc.errPart) || !strings.Contains(err.Error(), "pkg/Project.toml") {
					t.Fatalf("err = %v, want %q naming the path", err, tc.errPart)
				}
				return
			}
			if err != nil || p.Name != tc.want {
				t.Fatalf("name = %q, err = %v, want %q", p.Name, err, tc.want)
			}
		})
	}
}

func TestTestDeps(t *testing.T) {
	p, err := ParseProject("Project.toml", []byte("name = \"Demo\"\n[extras]\nTest = \""+testStd+"\"\nOther = \""+testUUID+"\"\n[targets]\ntest = [\"Test\"]\n"))
	if err != nil {
		t.Fatal(err)
	}
	if got := sortedKeys(p.TestDeps(nil)); strings.Join(got, ",") != "Test" {
		t.Errorf("extras deps = %v, want [Test]", got)
	}
	o, err := ParseProject("test/Project.toml", []byte("[deps]\nDemo = \""+testUUID+"\"\nOther = \""+testUUID+"\"\n"))
	if err != nil {
		t.Fatal(err)
	}
	if got := sortedKeys(p.TestDeps(&o)); strings.Join(got, ",") != "Other" {
		t.Errorf("test/Project.toml deps = %v, want [Other] without the package itself", got)
	}
}
