package astro

import "github.com/ralvik/rules_dx/gazelle/common"

var SupportedExts = []string{".astro"}

func ModuleName(name string) string { return common.Stem(name, SupportedExts) }

func TargetName(name string) (string, error) { return common.Normalize(ModuleName(name)) }
