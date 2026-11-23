package haskell

import (
	"path/filepath"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

var SupportedExts = []string{".hs"}

var TestSourceSuffixes = []string{"Spec", "Test", "Tests"}

const LibraryKind = "haskell_library"

func IsTestSource(name string) bool {
	return common.IsTestSource(name, SupportedExts, TestSourceSuffixes)
}

func isSupported(name string) bool { return common.HasExt(name, SupportedExts) }

func DirTargetName(dir string) (string, error) { return common.DirTargetName(dir) }

func Identity(name string) string { return common.Stem(name, SupportedExts) }

func declaredModule(path string) []string {
	if content, err := common.ReadSource(path); err == nil {
		if m := ParseModule(content); m != "" {
			return []string{m}
		}
	}
	return []string{Identity(filepath.Base(path))}
}

var stdlibPrefixes = []string{
	"Control", "Data", "Debug", "Foreign", "GHC", "Numeric", "System", "Text", "Type", "Unsafe",
}

var stdlibModules = map[string]bool{
	"Prelude": true, "Numeric": true, "Foreign": true, "Data": true,
}

func IsStdLib(module string) bool {
	if stdlibModules[module] {
		return true
	}
	for _, p := range stdlibPrefixes {
		if len(module) > len(p) && module[:len(p)] == p && module[len(p)] == '.' {
			switch p {
			case "Data", "Control", "Text", "System":
				return baseOnly(module)
			}
			return true
		}
	}
	return false
}

func baseOnly(module string) bool {
	for _, prefix := range baseModulePrefixes {
		if module == prefix || len(module) > len(prefix) && module[:len(prefix)] == prefix && module[len(prefix)] == '.' {
			return true
		}
	}
	return false
}

var baseModulePrefixes = []string{
	"Control.Applicative", "Control.Arrow", "Control.Category", "Control.Concurrent",
	"Control.Exception", "Control.Monad", "Control.Monad.ST", "Control.Monad.Fix",
	"Control.Monad.Fail", "Control.Monad.Zip", "Control.Monad.IO.Class",
	"Data.Bifunctor", "Data.Bits", "Data.Bool", "Data.Char", "Data.Coerce", "Data.Complex",
	"Data.Data", "Data.Dynamic", "Data.Either", "Data.Eq", "Data.Fixed", "Data.Foldable",
	"Data.Function", "Data.Functor", "Data.IORef", "Data.Int", "Data.Ix", "Data.Kind",
	"Data.List", "Data.Maybe", "Data.Monoid", "Data.Ord", "Data.Proxy", "Data.Ratio",
	"Data.STRef", "Data.Semigroup", "Data.String", "Data.Traversable", "Data.Tuple",
	"Data.Type", "Data.Typeable", "Data.Unique", "Data.Version", "Data.Void", "Data.Word",
	"System.Environment", "System.Exit", "System.IO", "System.Mem", "System.Timeout",
	"System.Info", "System.CPUTime", "System.Posix.Types",
	"Text.Printf", "Text.Read", "Text.Show",
}
