#include "cuda/testing/gpu_facts.h"

#include <cmath>
#include <cstdio>
#include <string>

namespace dx_gpu {
namespace {

bool ParseVersion(const std::string &text, int *major, int *minor) {
  size_t dot = text.find('.');
  if (dot == std::string::npos || dot == 0 || dot + 1 == text.size()) {
    return false;
  }
  int parts[2] = {0, 0};
  int part = 0;
  for (size_t i = 0; i < text.size(); ++i) {
    char c = text[i];
    if (i == dot) {
      part = 1;
    } else if (c >= '0' && c <= '9' && parts[part] < 100000) {
      parts[part] = parts[part] * 10 + (c - '0');
    } else {
      return false;
    }
  }
  *major = parts[0];
  *minor = parts[1];
  return true;
}

int Rank(int major, int minor) { return major * 1000 + minor; }

int EncodedRank(int encoded) {
  return Rank(encoded / 1000, (encoded % 1000) / 10);
}

std::string Quote(const std::string &text) {
  std::string out = "\"";
  for (unsigned char c : text) {
    if (c == '"') {
      out += "\\\"";
    } else if (c == '\\') {
      out += "\\\\";
    } else if (c == '\n') {
      out += "\\n";
    } else if (c == '\r') {
      out += "\\r";
    } else if (c == '\t') {
      out += "\\t";
    } else if (c < 0x20 || c == 0x7f) {
      char buf[8];
      std::snprintf(buf, sizeof(buf), "\\u%04x", c);
      out += buf;
    } else {
      out += static_cast<char>(c);
    }
  }
  return out + "\"";
}

std::string Number(double value) {
  if (!std::isfinite(value)) {
    return "null";
  }
  char buf[32];
  std::snprintf(buf, sizeof(buf), "%.9g", value);
  return buf;
}

std::string VersionOrNull(int encoded) {
  return encoded > 0 ? Quote(CudaVersion(encoded)) : "null";
}

} // namespace

const char *StatusName(Status status) {
  switch (status) {
  case Status::kPass:
    return "pass";
  case Status::kUnavailable:
    return "unavailable";
  case Status::kUnsupported:
    return "unsupported";
  case Status::kFail:
    break;
  }
  return "fail";
}

std::string CudaVersion(int encoded) {
  return std::to_string(encoded / 1000) + "." +
         std::to_string((encoded % 1000) / 10);
}

std::string DeclaredError(const Declared &declared) {
  int major = 0;
  int minor = 0;
  if (!ParseVersion(declared.min_compute_capability, &major, &minor)) {
    return "DX_GPU_MIN_CC is missing or not X.Y";
  }
  if (!ParseVersion(declared.min_driver, &major, &minor)) {
    return "DX_GPU_MIN_DRIVER is missing or not X.Y";
  }
  if (!ParseVersion(declared.runtime, &major, &minor)) {
    return "DX_GPU_RUNTIME is missing or not X.Y";
  }
  return "";
}

Verdict DriverVerdict(const Probe &probe, const Declared &declared) {
  int major = 0;
  int minor = 0;
  ParseVersion(declared.min_driver, &major, &minor);
  if (EncodedRank(probe.driver) < Rank(major, minor)) {
    return {Status::kUnsupported, "driver " + CudaVersion(probe.driver) +
                                      " is below " + declared.min_driver};
  }
  return {Status::kPass, ""};
}

Verdict CheckDevice(const Probe &probe, const Declared &declared) {
  std::string declared_error = DeclaredError(declared);
  if (!declared_error.empty()) {
    return {Status::kFail, declared_error};
  }
  if (!probe.error.empty()) {
    // The driver is read before the device count, so a failed count (such as
    // cudaErrorInsufficientDriver) still leaves a driver version to check.
    if (probe.driver > 0) {
      Verdict driver = DriverVerdict(probe, declared);
      if (driver.status != Status::kPass) {
        return driver;
      }
    }
    return {Status::kUnavailable, probe.error};
  }
  if (probe.device_count <= 0) {
    return {Status::kUnavailable, "no CUDA device"};
  }
  int major = 0;
  int minor = 0;
  ParseVersion(declared.min_compute_capability, &major, &minor);
  if (Rank(probe.cc_major, probe.cc_minor) < Rank(major, minor)) {
    return {Status::kUnsupported,
            "device compute capability " + std::to_string(probe.cc_major) +
                "." + std::to_string(probe.cc_minor) + " is below " +
                declared.min_compute_capability};
  }
  Verdict driver = DriverVerdict(probe, declared);
  if (driver.status != Status::kPass) {
    return driver;
  }
  ParseVersion(declared.runtime, &major, &minor);
  if (EncodedRank(probe.runtime) != Rank(major, minor)) {
    return {Status::kUnsupported, "runtime " + CudaVersion(probe.runtime) +
                                      " is not " + declared.runtime};
  }
  return {Status::kPass, ""};
}

Verdict Classify(const Probe &probe, const Declared &declared) {
  Verdict device = CheckDevice(probe, declared);
  if (device.status != Status::kPass) {
    return device;
  }
  if (!probe.ran) {
    return {Status::kFail, "dx_gpu_test_main did not run"};
  }
  if (probe.main_result != 0) {
    return {Status::kFail,
            "dx_gpu_test_main returned " + std::to_string(probe.main_result)};
  }
  if (probe.checks.empty()) {
    return {Status::kFail, "no checks recorded"};
  }
  for (const Check &check : probe.checks) {
    if (!check.ok) {
      return {Status::kFail, "check " + check.name + " failed"};
    }
  }
  return {Status::kPass,
          std::to_string(probe.checks.size()) + " checks passed"};
}

Check Compare(const char *name, const float *want, const float *got, int n,
              double abs_tol, double rel_tol) {
  Check check;
  check.name = name != nullptr ? name : "";
  check.n = n;
  check.abs_tol = abs_tol;
  check.rel_tol = rel_tol;
  if (want == nullptr || got == nullptr || n <= 0) {
    check.max_abs_error = NAN;
    return check;
  }
  bool ok = true;
  double max_error = 0.0;
  for (int i = 0; i < n; ++i) {
    double w = want[i];
    double g = got[i];
    double error = std::fabs(g - w);
    if (!std::isfinite(w) || !std::isfinite(g)) {
      ok = false;
      max_error = NAN;
      continue;
    }
    if (!(error <= abs_tol + rel_tol * std::fabs(w))) {
      ok = false;
    }
    if (error > max_error) {
      max_error = error;
    }
  }
  check.max_abs_error = max_error;
  check.ok = ok;
  return check;
}

std::string FactsJson(const Probe &probe, const Declared &declared,
                      const Verdict &verdict) {
  std::string out = "{\"schema\":\"dx.cuda.gpu_facts.v1\"";
  out += ",\"status\":" + Quote(StatusName(verdict.status));
  out += ",\"reason\":" + Quote(verdict.reason);
  out += ",\"declared\":{\"min_compute_capability\":" +
         Quote(declared.min_compute_capability) +
         ",\"min_driver\":" + Quote(declared.min_driver) +
         ",\"runtime\":" + Quote(declared.runtime) + "}";
  out += ",\"driver\":" + VersionOrNull(probe.driver);
  out += ",\"runtime\":" + VersionOrNull(probe.runtime);
  if (probe.error.empty() && probe.device_count > 0) {
    out += ",\"device\":{\"name\":" + Quote(probe.device_name) +
           ",\"compute_capability\":" +
           Quote(std::to_string(probe.cc_major) + "." +
                 std::to_string(probe.cc_minor)) +
           ",\"memory_bytes\":" + std::to_string(probe.memory_bytes) + "}";
  } else {
    out += ",\"device\":null";
  }
  out += ",\"checks\":[";
  for (size_t i = 0; i < probe.checks.size(); ++i) {
    const Check &check = probe.checks[i];
    if (i > 0) {
      out += ",";
    }
    out += "{\"name\":" + Quote(check.name) +
           ",\"n\":" + std::to_string(check.n) +
           ",\"max_abs_error\":" + Number(check.max_abs_error) +
           ",\"abs_tol\":" + Number(check.abs_tol) +
           ",\"rel_tol\":" + Number(check.rel_tol) +
           ",\"ok\":" + (check.ok ? "true" : "false") + "}";
  }
  return out + "]}";
}

} // namespace dx_gpu
