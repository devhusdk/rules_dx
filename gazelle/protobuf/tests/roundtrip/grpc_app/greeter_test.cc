#include <memory>
#include <string>

#include <gtest/gtest.h>
#include <grpcpp/grpcpp.h>

#include "gazelle/protobuf/tests/roundtrip/grpc_app/greeter.grpc.pb.h"

namespace {

class GreeterImpl final : public greeter::Greeter::Service {
 public:
  grpc::Status SayHello(grpc::ServerContext*, const greeter::HelloRequest* request,
                        greeter::HelloReply* reply) override {
    reply->set_message("hello " + request->name());
    return grpc::Status::OK;
  }
};

TEST(GreeterTest, InProcessCall) {
  GreeterImpl service;
  grpc::ServerBuilder builder;
  builder.RegisterService(&service);
  std::unique_ptr<grpc::Server> server = builder.BuildAndStart();
  ASSERT_NE(server, nullptr);

  auto stub = greeter::Greeter::NewStub(server->InProcessChannel(grpc::ChannelArguments()));
  greeter::HelloRequest request;
  request.set_name("dx");
  greeter::HelloReply reply;
  grpc::ClientContext context;
  ASSERT_TRUE(stub->SayHello(&context, request, &reply).ok());
  EXPECT_EQ(reply.message(), "hello dx");
  server->Shutdown();
}

}  // namespace
