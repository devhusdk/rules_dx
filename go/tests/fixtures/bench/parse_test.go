package bench

import "testing"

func BenchmarkParse(b *testing.B) {
	data := []byte("a,b,c")
	for i := 0; i < b.N; i++ {
		Parse(data)
	}
}
