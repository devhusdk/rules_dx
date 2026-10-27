package elixir

import (
	"os"
	"regexp"
	"sort"
	"strings"
	"testing"
)

func TestStdlibList(t *testing.T) {
	var names []string
	header := ""
	for _, line := range strings.Split(strings.TrimSpace(stdlibModulesText), "\n") {
		if strings.HasPrefix(line, "#") {
			header = line
			continue
		}
		names = append(names, line)
	}
	if !sort.StringsAreSorted(names) {
		t.Fatal("stdlib_modules.txt is not sorted")
	}
	for i := 1; i < len(names); i++ {
		if names[i] == names[i-1] {
			t.Fatalf("duplicate %s", names[i])
		}
	}
	for _, want := range []string{"Kernel", "GenServer", "ExUnit.Case", "Logger", "Mix.Project", "String.Chars"} {
		if !IsStdLib(want) {
			t.Errorf("IsStdLib(%q) = false", want)
		}
	}
	for _, not := range []string{"String.Utils", "Jason", "Elixir.Kernel"} {
		if IsStdLib(not) {
			t.Errorf("IsStdLib(%q) = true", not)
		}
	}
	pins, err := os.ReadFile("../../elixir/toolchain/pins.bzl")
	if err != nil {
		t.Fatal(err)
	}
	m := regexp.MustCompile(`ELIXIR_VERSION = "([^"]+)"`).FindSubmatch(pins)
	if m == nil {
		t.Fatal("no ELIXIR_VERSION in pins.bzl")
	}
	if !strings.HasPrefix(header, "# Elixir "+string(m[1])+":") {
		t.Fatalf("stdlib_modules.txt header %q does not match ELIXIR_VERSION %s; regenerate it", header, m[1])
	}
}

func TestCandidates(t *testing.T) {
	got := strings.Join(Candidates("Jason.Encoder.Any"), ",")
	if got != "Jason.Encoder,Jason" {
		t.Fatalf("Candidates = %s", got)
	}
	if c := Candidates("Jason"); len(c) != 0 {
		t.Fatalf("Candidates(Jason) = %v", c)
	}
}
