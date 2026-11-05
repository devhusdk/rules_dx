package main

import (
	"context"
	"fmt"
	"net"

	"google.golang.org/grpc"

	pb "example.com/grpc_test"
)

type greeter struct {
	pb.UnimplementedGreeterServer
}

func (greeter) SayHello(_ context.Context, req *pb.HelloRequest) (*pb.HelloReply, error) {
	return &pb.HelloReply{Message: "hello " + req.GetName()}, nil
}

func main() {
	lis, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		panic(err)
	}
	fmt.Println(lis.Addr().(*net.TCPAddr).Port)
	s := grpc.NewServer()
	pb.RegisterGreeterServer(s, greeter{})
	if err := s.Serve(lis); err != nil {
		panic(err)
	}
}
