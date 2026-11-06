import os
import subprocess

import grpc

import ping_pb2_grpc
from gazelle.protobuf.tests.grpc import greeter_pb2, greeter_pb2_grpc
from gazelle.protobuf.tests.grpc.v1 import echo_pb2_grpc


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


def test_stubs_from_subdir_and_stripped_protos():
    with grpc.insecure_channel("127.0.0.1:0") as channel:
        assert echo_pb2_grpc.EchoStub(channel).Say
        assert ping_pb2_grpc.PingStub(channel).Send
