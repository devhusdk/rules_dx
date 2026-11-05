package errcheck

import "os"

func Remove(path string) {
	_ = os.Remove(path)
}
