package rust

import (
	"fmt"
	"github.com/ralvik/rules_dx/gazelle/common"
	"strings"
)

func Normalize(base string) (string, error) {
	return common.Normalize(base)
}

type Claimant struct {
	Name   string
	Source string
}

type CollisionError struct {
	Name      string
	Claimants []string
}

func (e *CollisionError) Error() string {
	return fmt.Sprintf("naming: normalized name %q claimed by %s; rename a source or keep one target handwritten",
		e.Name, strings.Join(e.Claimants, ", "))
}

func CheckCollisions(claimants []Claimant) error {
	byName := make(map[string][]string, len(claimants))
	order := make([]string, 0, len(claimants))
	for _, c := range claimants {
		if _, ok := byName[c.Name]; !ok {
			order = append(order, c.Name)
		}
		byName[c.Name] = append(byName[c.Name], c.Source)
	}
	for _, name := range order {
		if sources := byName[name]; len(sources) > 1 {
			return &CollisionError{Name: name, Claimants: append([]string(nil), sources...)}
		}
	}
	return nil
}

func IntegrationTestName(stem string) string {
	if strings.HasSuffix(stem, "_test") {
		return stem
	}
	return stem + "_test"
}

func UnitTestName(crateTarget string) string {
	return crateTarget + "_test"
}

func BinaryName(crateName string) string {
	return crateName + "_bin"
}

func ExampleName(cargoName string) (string, error) {
	stem, err := Normalize(cargoName)
	if err != nil {
		return "", err
	}
	return stem + "_example", nil
}

func ExampleTestName(exampleTarget string) string {
	return exampleTarget + "_test"
}

func BenchName(cargoName string) (string, error) {
	stem, err := Normalize(cargoName)
	if err != nil {
		return "", err
	}
	return stem + "_bench", nil
}

func BuildScriptName(packageName string) (string, error) {
	stem, err := Normalize(packageName)
	if err != nil {
		return "", err
	}
	return stem + "_build_script", nil
}
