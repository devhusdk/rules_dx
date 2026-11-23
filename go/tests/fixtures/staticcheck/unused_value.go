// Package unusedvalue overwrites a value before it is read.
package unusedvalue

// Size returns twice the length of text.
func Size(text string) int {
	n := len(text)
	n = 2 * len(text)
	return n
}
