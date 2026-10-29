package errcheck

import "os"

func Persist(path string) {
	f, _ := os.Create(path)
	f.WriteString("hello")
	f.Close()
}
