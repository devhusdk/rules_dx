package errcheck

import (
	"fmt"
	"os"
)

func Persist(path string) error {
	f, err := os.Create(path)
	if err != nil {
		return err
	}
	fmt.Println("created", path)
	return f.Close()
}
