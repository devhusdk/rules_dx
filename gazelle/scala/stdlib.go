package scala

import "github.com/devhusdk/rules_dx/gazelle/common"

// platformPackages lists the packages inside scala-library 2.13.18, the
// version the repo pins in MODULE.bazel (from `unzip -Z1 scala-library.jar`).
// It is exact on purpose: scala.xml, scala.util.parsing and
// scala.collection.parallel are separate artifacts.
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
)

// externalPackages are packages that sit under a platform package but need a
// separate dependency, so a wildcard or member import of them is not stdlib.
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
