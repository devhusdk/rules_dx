package golint

import (
	"fmt"

	toml "github.com/pelletier/go-toml/v2"

	"rules_dx/go/tests/fixtures/golint/base"
)

func Clean(data []byte) error {
	if err := base.Do(); err != nil {
		return err
	}
	var v map[string]any
	if err := toml.Unmarshal(data, &v); err != nil {
		return err
	}
	fmt.Println(v)
	return nil
}
