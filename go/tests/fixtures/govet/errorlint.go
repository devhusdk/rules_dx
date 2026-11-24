package govet

import "errors"

var ErrMissing = errors.New("missing")

func IsMissing(err error) bool {
	return err == ErrMissing
}
