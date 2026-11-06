package main

import (
	"fmt"

	apiv1 "example.com/proto_bindings/api/v1"
	"google.golang.org/protobuf/proto"
)

func main() {
	data, err := proto.Marshal(&apiv1.Greeting{Name: "go", Times: 2})
	if err != nil {
		panic(err)
	}
	var back apiv1.Greeting
	if err := proto.Unmarshal(data, &back); err != nil {
		panic(err)
	}
	fmt.Println(back.GetName(), back.GetTimes())
}
