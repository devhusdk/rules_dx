package staticcheck

func double(n int) int {
	return n * 2
}

func Greet(name string) string {
	double(len(name))
	return "hello " + name
}

func unusedHelper() int {
	return 1
}
