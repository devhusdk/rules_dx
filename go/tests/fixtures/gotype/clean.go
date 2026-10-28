package clean

import (
	"fmt"
	"strings"

	"rules_dx/go/tests/fixtures/gotype/base"
)

func Greeting() string {
	return strings.ToUpper(fmt.Sprintf("hello %s", base.Name))
}
