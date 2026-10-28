package php

import "testing"

func TestTestTargetName(t *testing.T) {
	for src, want := range map[string]string{
		"tests/GreeterTest.php":     "greeter_test",
		"test/FooBarTest.php":       "foo_bar_test",
		"tests/Sub/HTTPClientTest.php": "http_client_test",
		"tests/V2ParserTest.php":    "v2_parser_test",
	} {
		got, err := TestTargetName(src)
		if err != nil || got != want {
			t.Errorf("TestTargetName(%q) = %q, %v, want %q", src, got, err, want)
		}
	}
}

func TestIsStdLib(t *testing.T) {
	for name, want := range map[string]bool{
		"Exception": true, "ArrayObject": true, "strlen": true, "PHP_EOL": true, "exception": true,
		`App\Exception`: false, "Greeter": false, `PHPUnit\Framework\TestCase`: false, "": false,
	} {
		if got := IsStdLib(name); got != want {
			t.Errorf("IsStdLib(%q) = %v, want %v", name, got, want)
		}
	}
}

func TestIsTestSource(t *testing.T) {
	for name, want := range map[string]bool{"FooTest.php": true, "Test.php": false, "Foo.php": false, "FooTest.phpt": false} {
		if got := isTestSource(name); got != want {
			t.Errorf("isTestSource(%q) = %v, want %v", name, got, want)
		}
	}
}
