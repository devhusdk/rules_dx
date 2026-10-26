package kotlin

import "github.com/devhusdk/rules_dx/gazelle/common"

// Packages in kotlin-stdlib 2.4.10.
var platformPackages = common.PackageSet(
	"kotlin",
	"kotlin.annotation",
	"kotlin.collections",
	"kotlin.collections.builders",
	"kotlin.collections.jdk8",
	"kotlin.collections.unsigned",
	"kotlin.comparisons",
	"kotlin.concurrent",
	"kotlin.concurrent.atomics",
	"kotlin.concurrent.internal",
	"kotlin.contracts",
	"kotlin.coroutines",
	"kotlin.coroutines.cancellation",
	"kotlin.coroutines.intrinsics",
	"kotlin.coroutines.jvm.internal",
	"kotlin.enums",
	"kotlin.experimental",
	"kotlin.internal",
	"kotlin.internal.jdk7",
	"kotlin.internal.jdk8",
	"kotlin.io",
	"kotlin.io.encoding",
	"kotlin.io.path",
	"kotlin.jdk7",
	"kotlin.js",
	"kotlin.jvm",
	"kotlin.jvm.functions",
	"kotlin.jvm.internal",
	"kotlin.jvm.internal.markers",
	"kotlin.jvm.internal.unsafe",
	"kotlin.jvm.jdk8",
	"kotlin.jvm.optionals",
	"kotlin.math",
	"kotlin.properties",
	"kotlin.random",
	"kotlin.random.jdk8",
	"kotlin.ranges",
	"kotlin.reflect",
	"kotlin.sequences",
	"kotlin.streams",
	"kotlin.streams.jdk8",
	"kotlin.system",
	"kotlin.text",
	"kotlin.text.jdk8",
	"kotlin.time",
	"kotlin.time.jdk8",
	"kotlin.uuid",
)

var externalPackages = []string{
	"kotlin.reflect.full",
	"kotlin.reflect.jvm",
	"kotlin.script",
	"kotlin.test",
}

func IsStdLib(spec string) bool {
	return !common.InPackages(spec, externalPackages) &&
		(common.InPlatformPackages(spec, platformPackages) || common.IsJDKPlatform(spec))
}
