package govet

// Sum adds up values.
func Sum(values []int) int {
	total := 0
	for _, v := range values {
		v := v
		total += v
	}
	return total
}
