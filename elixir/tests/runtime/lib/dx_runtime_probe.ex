defmodule DxRuntimeProbe do
  def versions, do: {System.version(), System.otp_release()}
end
