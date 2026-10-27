package haskell

import "github.com/devhusdk/rules_dx/gazelle/common"

// SupportedExts lists the Haskell source extensions the generator owns.
var SupportedExts = []string{".hs"}

// TestSourceSuffixes marks module stems that belong to tests, not the library.
var TestSourceSuffixes = []string{"Spec", "Test", "Tests"}

// LibraryKind is the generated rule kind.
const LibraryKind = "haskell_library"

// IsTestSource reports whether name is a Haskell test module.
func IsTestSource(name string) bool {
	return common.IsTestSource(name, SupportedExts, TestSourceSuffixes)
}

func isSupported(name string) bool { return common.HasExt(name, SupportedExts) }

// DirTargetName names the library generated for dir.
func DirTargetName(dir string) (string, error) { return common.DirTargetName(dir) }

// Identity maps a source file to the module name it is indexed under.
func Identity(name string) string { return common.Stem(name, SupportedExts) }

var stdlibPrefixes = []string{
	"Control", "Data", "Debug", "Foreign", "GHC", "Numeric", "System", "Text", "Type", "Unsafe",
}

var stdlibModules = map[string]bool{
	"Prelude": true, "Numeric": true, "Foreign": true, "Data": true,
}

// IsStdLib reports whether module ships with GHC's base library tree.
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

// baseOnly narrows the broad prefixes to modules base, containers-free GHC
// boot libraries, and friends actually ship, so third-party packages that
// reuse a prefix (Data.Aeson, Control.Lens) still resolve as dependencies.
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
