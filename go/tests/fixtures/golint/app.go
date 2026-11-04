package golint

import (
	"fmt"

	toml "github.com/pelletier/go-toml/v2"

	"rules_dx/go/tests/fixtures/golint/base"
)

func Run(data []byte) {
	base.Do()
	var v map[string]any
	toml.Unmarshal(data, &v)
	fmt.Printf(format)
}
