package root

import "testing"

func TestName(t *testing.T) {
	t.Parallel()
	if Name() != "root" {
		t.Fatal(Name())
	}
}
