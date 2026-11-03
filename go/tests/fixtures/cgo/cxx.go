package cgo

// #include "adder.h"
// #ifndef DX_CGO_COPT
// #error DX_CGO_COPT is not set
// #endif
import "C"

// CxxAdd returns a + b, computed by C++ code.
func CxxAdd(a, b int) int {
	return int(C.dx_cxx_add(C.int(a), C.int(b)))
}
