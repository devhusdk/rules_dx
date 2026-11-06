#ifndef PROTO_BINDINGS_CC_GREETER_H_
#define PROTO_BINDINGS_CC_GREETER_H_

#include <string>

#include "api/v1/greeting.pb.h"

bool RoundTrip(const std::string& name, int times, api::v1::Greeting* out);

#endif
