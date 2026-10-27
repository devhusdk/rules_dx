package pkg

import (
	"github.com/google/go-cmp/cmp"
	"gopkg.in/yaml.v3"
)

func Same(a, b any) bool {
	_, err := yaml.Marshal(a)
	return err == nil && cmp.Equal(a, b)
}
