package bench

func Parse(data []byte) int {
	n := 0
	for _, b := range data {
		if b == ',' {
			n++
		}
	}
	return n
}
