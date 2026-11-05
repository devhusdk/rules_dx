//go:build !dx_off

package golint

func double(n int) int {
	return n * 2
}

func Tagged(name string) {
	double(len(name))
}
