import os
import subprocess

import grpc

from gazelle.protobuf.tests.grpc import greeter_pb2, greeter_pb2_grpc


def test_unary_call_to_go_server():
    server = subprocess.Popen([os.environ["GREETER_SERVER"]], stdout=subprocess.PIPE, text=True)
    try:
        port = int(server.stdout.readline())
        with grpc.insecure_channel(f"127.0.0.1:{port}") as channel:
            stub = greeter_pb2_grpc.GreeterStub(channel)
            reply = stub.SayHello(greeter_pb2.HelloRequest(name="dx"), timeout=30)
        assert reply.message == "hello dx"
    finally:
        server.kill()
        server.wait()
