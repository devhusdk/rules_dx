package quality.testdata.jvm_owned

import quality.testdata.jvm_owned.kept.KtKeptHelper

object KotlinUnusedDep {
    fun value(): Int = KtKeptHelper.value()
}
