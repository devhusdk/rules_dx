package cgo

// #include "add.h"
import "C"

func Hello(name string) string {
	return "hello " + name
}

// Add returns a + b, computed by the C helper in add.c.
func Add(a, b int) int {
	return int(C.add(C.int(a), C.int(b)))
}
