package hello

import "testing"
import "example.com/greet"
import "example.com/testhelper"

func TestHello(t *testing.T) { if greet.Hello("w") == "" { t.Fail() }; _ = testhelper.Check("w") }
