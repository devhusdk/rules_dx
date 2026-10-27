package haskell

import (
	"reflect"
	"testing"
)

func TestParseImports(t *testing.T) {
	src := "module Greet where\n" +
		"import Data.List (sort)\n" +
		"import qualified Data.Map as M\n" +
		"import safe \"pkg\" Util.Text\n" +
		"-- import Commented\n" +
		"{- import Block\n{- nested -} import Block2 -}\n" +
		"import Local\n"
	got := ParseImports([]byte(src))
	want := []string{"Data.List", "Data.Map", "Local", "Util.Text"}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("got %v, want %v", got, want)
	}
}

func TestDefinesMain(t *testing.T) {
	for src, want := range map[string]bool{
		"module Main where\n":          true,
		"main :: IO ()\nmain = pure ()": true,
		"module Greet where\n-- main = 1\n": false,
		"module Greet where\nmainly = 1\n":  false,
	} {
		if got := DefinesMain([]byte(src)); got != want {
			t.Errorf("DefinesMain(%q) = %v, want %v", src, got, want)
		}
	}
}

func TestIsStdLib(t *testing.T) {
	for name, want := range map[string]bool{
		"Prelude": true, "Data.List": true, "Control.Monad.State": true,
		"GHC.Generics": true, "Data.Aeson": false, "Control.Lens": false, "Util": false,
	} {
		if got := IsStdLib(name); got != want {
			t.Errorf("IsStdLib(%q) = %v, want %v", name, got, want)
		}
	}
}

func TestNaming(t *testing.T) {
	if !IsTestSource("GreetSpec.hs") || IsTestSource("Greet.hs") {
		t.Error("test source detection")
	}
	if Identity("Greet.hs") != "Greet" {
		t.Error("identity")
	}
}
