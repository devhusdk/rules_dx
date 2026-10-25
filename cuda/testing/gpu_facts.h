#ifndef DX_CUDA_TESTING_GPU_FACTS_H_
#define DX_CUDA_TESTING_GPU_FACTS_H_

#include <string>
#include <vector>

namespace dx_gpu {

enum class Status { kPass, kFail, kUnavailable, kUnsupported };

const char *StatusName(Status status);

struct Declared {
  std::string min_compute_capability;
  std::string min_driver;
  std::string runtime;
};

struct Check {
  std::string name;
  int n = 0;
  double max_abs_error = 0.0;
  double abs_tol = 0.0;
  double rel_tol = 0.0;
  bool ok = false;
};

struct Probe {
  std::string error;
  int device_count = 0;
  int driver = 0;
  int runtime = 0;
  std::string device_name;
  int cc_major = 0;
  int cc_minor = 0;
  unsigned long long memory_bytes = 0;
  bool ran = false;
  int main_result = 0;
  std::vector<Check> checks;
};

struct Verdict {
  Status status = Status::kFail;
  std::string reason;
};

std::string DeclaredError(const Declared &declared);

Verdict CheckDevice(const Probe &probe, const Declared &declared);

Verdict Classify(const Probe &probe, const Declared &declared);

Check Compare(const char *name, const float *want, const float *got, int n,
              double abs_tol, double rel_tol);

std::string CudaVersion(int encoded);

std::string FactsJson(const Probe &probe, const Declared &declared,
                      const Verdict &verdict);

} // namespace dx_gpu

#endif
