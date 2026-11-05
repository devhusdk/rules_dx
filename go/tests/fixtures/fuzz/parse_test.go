package fuzz

import "testing"

func FuzzParse(f *testing.F) {
	f.Add([]byte("ok"))
	f.Fuzz(func(t *testing.T, data []byte) {
		Parse(data)
	})
}
