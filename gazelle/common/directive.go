package common

import (
	"fmt"
	"strconv"
	"strings"
)

// ParseBoolDirective reads the value of a true/false directive.
func ParseBoolDirective(value string) (bool, error) {
	on, err := strconv.ParseBool(strings.TrimSpace(value))
	if err != nil {
		return false, fmt.Errorf("want true or false, got %q", value)
	}
	return on, nil
}
