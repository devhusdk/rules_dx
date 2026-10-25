#include <cuda_runtime.h>

#include <cstdio>
#include <cstdlib>
#include <string>
#include <vector>

#include "cuda/testing/gpu_facts.h"
#include "cuda/testing/gpu_test.h"

namespace {

std::vector<dx_gpu::Check>& Checks() {
  static std::vector<dx_gpu::Check> checks;
  return checks;
}

std::string Env(const char* name) {
  const char* value = std::getenv(name);
  return value != nullptr ? value : "";
}

bool Failed(cudaError_t status, const char* call, dx_gpu::Probe* probe) {
  if (status == cudaSuccess) {
    return false;
  }
  probe->error = std::string(call) + ": " + cudaGetErrorName(status);
  return true;
}

void ProbeDevice(dx_gpu::Probe* probe) {
  if (Failed(cudaRuntimeGetVersion(&probe->runtime), "cudaRuntimeGetVersion",
             probe) ||
      Failed(cudaDriverGetVersion(&probe->driver), "cudaDriverGetVersion",
             probe) ||
      Failed(cudaGetDeviceCount(&probe->device_count), "cudaGetDeviceCount",
             probe) ||
      probe->device_count <= 0) {
    return;
  }
  cudaDeviceProp props;
  if (Failed(cudaGetDeviceProperties(&props, 0), "cudaGetDeviceProperties",
             probe)) {
    return;
  }
  probe->device_name = props.name;
  probe->cc_major = props.major;
  probe->cc_minor = props.minor;
  probe->memory_bytes = props.totalGlobalMem;
}

bool WriteFacts(const std::string& json) {
  std::string dir = Env("TEST_UNDECLARED_OUTPUTS_DIR");
  if (dir.empty()) {
    return true;
  }
  std::string path = dir + "/gpu_facts.json";
  std::FILE* file = std::fopen(path.c_str(), "w");
  if (file == nullptr) {
    return false;
  }
  bool ok = std::fputs(json.c_str(), file) >= 0 && std::fputc('\n', file) >= 0;
  return std::fclose(file) == 0 && ok;
}

}  // namespace

extern "C" bool dx_gpu_check(const char* name, const float* want,
                             const float* got, int n, double abs_tol,
                             double rel_tol) {
  dx_gpu::Check check = dx_gpu::Compare(name, want, got, n, abs_tol, rel_tol);
  Checks().push_back(check);
  return check.ok;
}

int main() {
  dx_gpu::Declared declared{Env("DX_GPU_MIN_CC"), Env("DX_GPU_MIN_DRIVER"),
                            Env("DX_GPU_RUNTIME")};
  dx_gpu::Probe probe;
  if (dx_gpu::DeclaredError(declared).empty()) {
    ProbeDevice(&probe);
  }
  if (dx_gpu::CheckDevice(probe, declared).status == dx_gpu::Status::kPass) {
    probe.main_result = dx_gpu_test_main();
    probe.ran = true;
    probe.checks = Checks();
  }
  dx_gpu::Verdict verdict = dx_gpu::Classify(probe, declared);
  std::string json = dx_gpu::FactsJson(probe, declared, verdict);
  std::printf("dx-gpu-facts: %s\n", json.c_str());
  std::fflush(stdout);
  if (!WriteFacts(json)) {
    std::fprintf(stderr, "fail: cannot write gpu_facts.json\n");
    return 1;
  }
  if (verdict.status != dx_gpu::Status::kPass) {
    std::fprintf(stderr, "%s: %s\n", dx_gpu::StatusName(verdict.status),
                 verdict.reason.c_str());
    return 1;
  }
  return 0;
}
