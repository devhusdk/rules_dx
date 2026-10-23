defmodule DxRuntimeProbeTest do
  use ExUnit.Case, async: true

  test "runs on the pinned Elixir and OTP" do
    assert DxRuntimeProbe.versions() ==
             {System.fetch_env!("DX_ELIXIR_VERSION"), System.fetch_env!("DX_OTP_RELEASE")}
  end

  test "loads the app from its compiled ebin" do
    assert Application.load(:dx_runtime_probe) == :ok
    assert Application.spec(:dx_runtime_probe, :modules) == [DxRuntimeProbe]
    assert :code.which(DxRuntimeProbe) |> to_string() |> String.ends_with?("/ebin/Elixir.DxRuntimeProbe.beam")
  end

  test "runs Elixir from the declared runtime, not the host" do
    elixir = System.find_executable("elixir")
    assert elixir =~ "dx_elixir/bin/elixir"
    assert :code.root_dir() |> to_string() |> String.contains?("dx_otp_")
  end
end
