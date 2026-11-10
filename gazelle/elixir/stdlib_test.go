package elixir

import (
	"encoding/json"
	"os"
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
	data, err := os.ReadFile("../../elixir/private/toolchain/pins.json")
	if err != nil {
		t.Fatal(err)
	}
	var pins struct {
		Version string `json:"version"`
	}
	if err := json.Unmarshal(data, &pins); err != nil {
		t.Fatal(err)
	}
	if !strings.HasPrefix(header, "# Elixir "+pins.Version+":") {
		t.Fatalf("stdlib_modules.txt header %q does not match pins.json version %s; regenerate it", header, pins.Version)
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
