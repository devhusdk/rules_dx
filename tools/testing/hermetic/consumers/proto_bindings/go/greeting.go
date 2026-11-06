package greeting

import (
	apiv1 "example.com/proto_bindings/api/v1"
	"google.golang.org/protobuf/proto"
)

func RoundTrip(name string, times int32) (*apiv1.Greeting, error) {
	data, err := proto.Marshal(&apiv1.Greeting{Name: name, Times: times})
	if err != nil {
		return nil, err
	}
	var back apiv1.Greeting
	if err := proto.Unmarshal(data, &back); err != nil {
		return nil, err
	}
	return &back, nil
}
