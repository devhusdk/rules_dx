//go:build windows

package hello

import "example.com/winonly"

func Platform() string { return winonly.Label() }
