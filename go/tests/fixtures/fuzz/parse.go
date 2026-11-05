package fuzz

func Parse(data []byte) int {
	if len(data) > 0 && data[0] == '!' {
		panic("planted crash")
	}
	return len(data)
}
