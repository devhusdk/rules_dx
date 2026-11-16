// Package lintdirty has one staticcheck finding.
package lintdirty

func double(n int) int {
	return n * 2
}

// Greet returns a greeting for name.
func Greet(name string) string {
	double(len(name))
	return "hello " + name
}
