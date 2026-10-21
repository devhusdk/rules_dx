package mdx

import "github.com/devhusdk/rules_dx/gazelle/common"

var SupportedExts = []string{".mdx"}

func ModuleName(name string) string { return common.Stem(name, SupportedExts) }

func TargetName(name string) (string, error) { return common.Normalize(ModuleName(name)) }
