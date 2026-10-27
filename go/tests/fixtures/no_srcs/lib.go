// Package nosrcs seeds a go_test that embeds a library and has no srcs.
package nosrcs

// Answer returns a constant.
func Answer() int {
	return 42
}
