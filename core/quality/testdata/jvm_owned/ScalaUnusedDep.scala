package quality.testdata.jvm_owned

import quality.testdata.jvm_owned.kept.ScKeptHelper

object ScalaUnusedDep {
  def value: Int = ScKeptHelper.value
}
