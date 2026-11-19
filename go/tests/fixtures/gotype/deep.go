package deep

import "rules_dx/go/tests/fixtures/gotype/mid"

func Label() string {
	return mid.NewBox().Label()
}
