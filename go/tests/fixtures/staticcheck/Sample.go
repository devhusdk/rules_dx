package staticcheck

import "fmt"

func Greet(name string) string {
	fmt.Sprintf("hello %s", name)
	return "hello " + name
}
