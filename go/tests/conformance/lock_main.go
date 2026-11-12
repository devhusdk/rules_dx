package main

import (
	"fmt"

	"github.com/google/go-cmp/cmp"
)

func main() {
	if cmp.Equal("lock", "lock") {
		fmt.Println("hello lock")
	}
}
