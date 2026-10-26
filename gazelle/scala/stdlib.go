package scala

import "github.com/devhusdk/rules_dx/gazelle/common"

// Packages in scala-library 2.13.18.
var platformPackages = common.PackageSet(
	"scala",
	"scala.annotation",
	"scala.annotation.meta",
	"scala.annotation.unchecked",
	"scala.beans",
	"scala.collection",
	"scala.collection.concurrent",
	"scala.collection.convert",
	"scala.collection.convert.impl",
	"scala.collection.generic",
	"scala.collection.immutable",
	"scala.collection.mutable",
	"scala.compat",
	"scala.concurrent",
	"scala.concurrent.duration",
	"scala.concurrent.impl",
	"scala.io",
	"scala.jdk",
	"scala.jdk.javaapi",
	"scala.math",
	"scala.ref",
	"scala.reflect",
	"scala.reflect.macros.internal",
	"scala.runtime",
	"scala.runtime.java8",
	"scala.sys",
	"scala.sys.process",
	"scala.util",
	"scala.util.control",
	"scala.util.hashing",
	"scala.util.matching",
	"scala.Predef",
	"scala.language",
	"scala.language.experimental",
	"scala.util.chaining",
)

var externalPackages = []string{
	"scala.collection.parallel",
	"scala.reflect.api",
	"scala.reflect.runtime",
	"scala.tools",
	"scala.util.parsing",
	"scala.xml",
}

func IsStdLib(spec string) bool {
	return !common.InPackages(spec, externalPackages) &&
		(common.InPlatformPackages(spec, platformPackages) || common.IsJDKPlatform(spec))
}
