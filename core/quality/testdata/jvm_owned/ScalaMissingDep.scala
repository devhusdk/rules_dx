package quality.testdata.jvm_owned

import quality.testdata.jvm_owned.kept.ScKeptHelper

object ScalaMissingDep {
  def value: Int = ScWrapper.value + ScKeptHelper.value
}
