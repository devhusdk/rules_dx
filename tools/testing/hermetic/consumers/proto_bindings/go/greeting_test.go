package greeting

import "testing"

func TestRoundTrip(t *testing.T) {
	got, err := RoundTrip("go", 2)
	if err != nil {
		t.Fatal(err)
	}
	if got.GetName() != "go" || got.GetTimes() != 2 {
		t.Fatalf("got %v", got)
	}
}
