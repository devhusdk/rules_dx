package broken

import "testing"

func TestName(t *testing.T) {
	if Name() == 1 {
		t.Fatal("unexpected")
	}
}
