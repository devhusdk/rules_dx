package main

import (
	"log"
	"os"

	apiv1 "example.com/proto_bindings/api/v1"
	"google.golang.org/protobuf/proto"
)

func main() {
	data, err := proto.Marshal(&apiv1.Greeting{Name: "from go", Times: 3})
	if err != nil {
		log.Fatal(err)
	}
	if _, err := os.Stdout.Write(data); err != nil {
		log.Fatal(err)
	}
}
