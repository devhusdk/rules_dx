package example.grpcjava;

import static org.junit.Assert.assertEquals;

import io.grpc.ManagedChannel;
import io.grpc.Server;
import io.grpc.inprocess.InProcessChannelBuilder;
import io.grpc.inprocess.InProcessServerBuilder;
import io.grpc.stub.StreamObserver;
import org.junit.Test;

public class GreeterTest {
  static class Impl extends GreeterGrpc.GreeterImplBase {
    @Override
    public void sayHello(HelloRequest req, StreamObserver<HelloReply> out) {
      out.onNext(HelloReply.newBuilder().setMessage("hello " + req.getName()).build());
      out.onCompleted();
    }
  }

  @Test
  public void sayHello() throws Exception {
    Server server =
        InProcessServerBuilder.forName("greeter").directExecutor().addService(new Impl()).build().start();
    ManagedChannel channel = InProcessChannelBuilder.forName("greeter").directExecutor().build();
    try {
      HelloReply reply =
          GreeterGrpc.newBlockingStub(channel)
              .sayHello(HelloRequest.newBuilder().setName("world").build());
      assertEquals("hello world", reply.getMessage());
    } finally {
      channel.shutdownNow();
      server.shutdownNow();
    }
  }
}
