package samepkg

import (
	"testing"

	"example.test/samepkg/helper"
)

func TestSamePackageExternalDep(t *testing.T) {
	t.Parallel()
	if got := Name(); got != "samepkg" {
		t.Fatalf("Name() = %q", got)
	}
	if got := helper.Name(); got != "helper" {
		t.Fatalf("helper.Name() = %q", got)
	}
}
