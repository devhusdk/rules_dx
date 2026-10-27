#include "cuda/testing/gpu_facts.h"

#include <cmath>
#include <string>

#include "gtest/gtest.h"

namespace dx_gpu {
namespace {

Declared Saxpy() { return Declared{"8.9", "13.0", "13.4"}; }

Probe Rtx() {
  Probe probe;
  probe.device_count = 1;
  probe.driver = 13040;
  probe.runtime = 13040;
  probe.device_name = "NVIDIA GeForce RTX 5070 Ti";
  probe.cc_major = 12;
  probe.cc_minor = 0;
  probe.memory_bytes = 17094475776ULL;
  probe.ran = true;
  Check check;
  check.name = "saxpy";
  check.n = 4;
  check.ok = true;
  probe.checks.push_back(check);
  return probe;
}

TEST(Classify, PassesWhenEveryCheckPasses) {
  Verdict verdict = Classify(Rtx(), Saxpy());
  EXPECT_EQ(verdict.status, Status::kPass);
  EXPECT_EQ(verdict.reason, "1 checks passed");
}

TEST(Classify, FailsOnAFailedCheck) {
  Probe probe = Rtx();
  probe.checks[0].ok = false;
  Verdict verdict = Classify(probe, Saxpy());
  EXPECT_EQ(verdict.status, Status::kFail);
  EXPECT_EQ(verdict.reason, "check saxpy failed");
}

TEST(Classify, FailsOnANonZeroTestMain) {
  Probe probe = Rtx();
  probe.main_result = 3;
  EXPECT_EQ(Classify(probe, Saxpy()).reason, "dx_gpu_test_main returned 3");
}

TEST(Classify, FailsWithZeroChecks) {
  Probe probe = Rtx();
  probe.checks.clear();
  Verdict verdict = Classify(probe, Saxpy());
  EXPECT_EQ(verdict.status, Status::kFail);
  EXPECT_EQ(verdict.reason, "no checks recorded");
}

TEST(Classify, FailsWhenTheTestMainDidNotRun) {
  Probe probe = Rtx();
  probe.ran = false;
  EXPECT_EQ(Classify(probe, Saxpy()).status, Status::kFail);
}

TEST(Classify, FailsOnAMissingDeclaration) {
  Declared declared = Saxpy();
  declared.min_driver = "";
  Verdict verdict = Classify(Rtx(), declared);
  EXPECT_EQ(verdict.status, Status::kFail);
  EXPECT_EQ(verdict.reason, "DX_GPU_MIN_DRIVER is missing or not X.Y");
  declared = Saxpy();
  declared.min_compute_capability = "8.x";
  EXPECT_EQ(Classify(Rtx(), declared).reason,
            "DX_GPU_MIN_CC is missing or not X.Y");
  declared = Saxpy();
  declared.runtime = "13";
  EXPECT_EQ(Classify(Rtx(), declared).reason,
            "DX_GPU_RUNTIME is missing or not X.Y");
}

TEST(Classify, NoDeviceIsUnavailable) {
  Probe probe = Rtx();
  probe.device_count = 0;
  Verdict verdict = Classify(probe, Saxpy());
  EXPECT_EQ(verdict.status, Status::kUnavailable);
  EXPECT_EQ(verdict.reason, "no CUDA device");
}

TEST(Classify, AFailingCudaCallIsUnavailable) {
  Probe probe = Rtx();
  probe.error = "cudaGetDeviceCount: cudaErrorInsufficientDriver";
  Verdict verdict = Classify(probe, Saxpy());
  EXPECT_EQ(verdict.status, Status::kUnavailable);
  EXPECT_EQ(verdict.reason, "cudaGetDeviceCount: cudaErrorInsufficientDriver");
}

TEST(Classify, AFailingCudaCallWithOldDriverIsUnsupported) {
  Probe probe = Rtx();
  probe.driver = 12090;
  probe.error = "cudaGetDeviceCount: cudaErrorInsufficientDriver";
  Verdict verdict = Classify(probe, Saxpy());
  EXPECT_EQ(verdict.status, Status::kUnsupported);
  EXPECT_EQ(verdict.reason, "driver 12.9 is below 13.0");
}

TEST(Classify, LowComputeCapabilityIsUnsupported) {
  Probe probe = Rtx();
  probe.cc_major = 8;
  probe.cc_minor = 6;
  Verdict verdict = Classify(probe, Saxpy());
  EXPECT_EQ(verdict.status, Status::kUnsupported);
  EXPECT_EQ(verdict.reason, "device compute capability 8.6 is below 8.9");
  probe.cc_minor = 9;
  EXPECT_EQ(Classify(probe, Saxpy()).status, Status::kPass);
}

TEST(Classify, LowDriverIsUnsupported) {
  Probe probe = Rtx();
  probe.driver = 12090;
  Verdict verdict = Classify(probe, Saxpy());
  EXPECT_EQ(verdict.status, Status::kUnsupported);
  EXPECT_EQ(verdict.reason, "driver 12.9 is below 13.0");
}

TEST(Classify, OtherRuntimeIsUnsupported) {
  Probe probe = Rtx();
  probe.runtime = 13000;
  Verdict verdict = Classify(probe, Saxpy());
  EXPECT_EQ(verdict.status, Status::kUnsupported);
  EXPECT_EQ(verdict.reason, "runtime 13.0 is not 13.4");
}

TEST(Classify, CheckDeviceIgnoresTheRun) {
  Probe probe = Rtx();
  probe.ran = false;
  probe.checks.clear();
  EXPECT_EQ(CheckDevice(probe, Saxpy()).status, Status::kPass);
}

TEST(Compare, AcceptsTheToleranceBoundary) {
  const float want[] = {1.0f, 100.0f};
  const float got[] = {1.5f, 110.5f};
  Check check = Compare("edge", want, got, 2, 0.5, 0.1);
  EXPECT_TRUE(check.ok);
  EXPECT_DOUBLE_EQ(check.max_abs_error, 10.5);
  EXPECT_EQ(check.n, 2);
}

TEST(Compare, RejectsBeyondTheTolerance) {
  const float want[] = {100.0f};
  const float got[] = {110.75f};
  Check check = Compare("over", want, got, 1, 0.5, 0.1);
  EXPECT_FALSE(check.ok);
  EXPECT_DOUBLE_EQ(check.max_abs_error, 10.75);
}

TEST(Compare, RejectsNaN) {
  const float want[] = {1.0f, 2.0f};
  const float got[] = {NAN, 2.0f};
  Check check = Compare("nan", want, got, 2, 1.0, 1.0);
  EXPECT_FALSE(check.ok);
  EXPECT_TRUE(std::isnan(check.max_abs_error));
}

TEST(Compare, RejectsAMissingResult) {
  const float want[] = {1.0f};
  EXPECT_FALSE(Compare("launch", want, nullptr, 1, 1.0, 1.0).ok);
  EXPECT_FALSE(Compare("empty", want, want, 0, 1.0, 1.0).ok);
}

TEST(FactsJson, WritesEveryField) {
  Probe probe = Rtx();
  probe.checks[0].max_abs_error = 0.25;
  probe.checks[0].abs_tol = 1e-05;
  probe.checks[0].rel_tol = 1e-06;
  std::string json = FactsJson(probe, Saxpy(), Classify(probe, Saxpy()));
  EXPECT_EQ(json,
            "{\"schema\":\"dx.cuda.gpu_facts.v1\",\"status\":\"pass\","
            "\"reason\":\"1 checks passed\",\"declared\":{"
            "\"min_compute_capability\":\"8.9\",\"min_driver\":\"13.0\","
            "\"runtime\":\"13.4\"},\"driver\":\"13.4\",\"runtime\":\"13.4\","
            "\"device\":{\"name\":\"NVIDIA GeForce RTX 5070 Ti\","
            "\"compute_capability\":\"12.0\",\"memory_bytes\":17094475776},"
            "\"checks\":[{\"name\":\"saxpy\",\"n\":4,\"max_abs_error\":0.25,"
            "\"abs_tol\":1e-05,\"rel_tol\":1e-06,\"ok\":true}]}");
}

TEST(FactsJson, EscapesTheDeviceName) {
  Probe probe = Rtx();
  probe.device_name = "a\"b\\c\nd\x01";
  std::string json = FactsJson(probe, Saxpy(), Classify(probe, Saxpy()));
  EXPECT_NE(json.find("\"name\":\"a\\\"b\\\\c\\nd\\u0001\""),
            std::string::npos);
}

TEST(FactsJson, WritesNullsWithoutADevice) {
  Probe probe;
  probe.error = "cudaGetDeviceCount: cudaErrorNoDevice";
  std::string json = FactsJson(probe, Saxpy(), Classify(probe, Saxpy()));
  EXPECT_NE(json.find("\"status\":\"unavailable\""), std::string::npos);
  EXPECT_NE(json.find("\"driver\":null,\"runtime\":null,\"device\":null,"
                      "\"checks\":[]"),
            std::string::npos);
}

TEST(FactsJson, WritesNullForNaN) {
  Probe probe = Rtx();
  probe.checks[0].max_abs_error = NAN;
  std::string json = FactsJson(probe, Saxpy(), Classify(probe, Saxpy()));
  EXPECT_NE(json.find("\"max_abs_error\":null"), std::string::npos);
}

TEST(StatusName, NamesEveryStatus) {
  EXPECT_STREQ(StatusName(Status::kPass), "pass");
  EXPECT_STREQ(StatusName(Status::kFail), "fail");
  EXPECT_STREQ(StatusName(Status::kUnavailable), "unavailable");
  EXPECT_STREQ(StatusName(Status::kUnsupported), "unsupported");
}

} // namespace
} // namespace dx_gpu
