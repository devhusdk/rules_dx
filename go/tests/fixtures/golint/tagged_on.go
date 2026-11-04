//go:build !dx_off

package golint

import "fmt"

func Tagged(name string) {
	fmt.Sprintf("hello %s", name)
}
