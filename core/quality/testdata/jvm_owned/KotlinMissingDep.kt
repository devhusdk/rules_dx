package quality.testdata.jvm_owned

import quality.testdata.jvm_owned.kept.KtKeptHelper

object KotlinMissingDep {
    fun value(): Int = KtWrapper.value() + KtKeptHelper.value()
}
