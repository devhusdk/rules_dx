package govet

import "strconv"

func Parse(text string) (int, error) {
	n, err := strconv.Atoi(text)
	if err == nil {
		n, err := strconv.Atoi(text + "0")
		if err != nil {
			return 0, err
		}
		_ = n
	}
	return n, err
}
