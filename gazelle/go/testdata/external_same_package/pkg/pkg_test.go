package pkg

import (
	"testing"

	"example.test/ext/helper"
	"example.test/ext/other"
)

func TestName(t *testing.T) {
	if Name() == "" || helper.Name() == "" || other.Name() == "" {
		t.Fatal("empty name")
	}
}
