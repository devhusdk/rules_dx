package swift

import (
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

func TestAppKind(t *testing.T) {
	for src, want := range map[string]string{
		"import UIKit\n@main\nclass A: UIResponder, UIApplicationDelegate {}\n": IOSAppKind,
		"import AppKit\n@main\nstruct A { static func main() {} }\n":            MacOSAppKind,
		"import UIKit\nimport AppKit\n@main\nstruct A {}\n":                     "",
		"import UIKit\nstruct A {}\n":                                           "",
		"import SwiftUI\n@main\nstruct A {}\n":                                  "",
	} {
		if got := AppKind([]byte(src)); got != want {
			t.Errorf("AppKind(%q) = %q, want %q", src, got, want)
		}
	}
}

func TestBundleID(t *testing.T) {
	if got := BundleID("my_app"); got != "com.example.my-app" {
		t.Errorf("BundleID = %q", got)
	}
}

func TestAppRule(t *testing.T) {
	ios := appRule(IOSAppKind, "phone")
	if ios.Name() != "phone_app" || ios.AttrString("entry") != ":phone" || ios.AttrString("platform") != iosPlatform {
		t.Errorf("iOS app = %s %q %q", ios.Name(), ios.AttrString("entry"), ios.AttrString("platform"))
	}
	if mac := appRule(MacOSAppKind, "desk"); mac.Attr("platform") != nil || mac.AttrString("bundle_id") != "com.example.desk" {
		t.Errorf("macOS app platform = %v, bundle_id = %q", mac.Attr("platform"), mac.AttrString("bundle_id"))
	}
}

func TestAppIsEmptyWithoutAnEntryPoint(t *testing.T) {
	for attr, value := range map[string]any{"entry": ":phone", "library": ":phone_stub", "srcs": []string{"main.m"}} {
		app := rule.NewRule(IOSAppKind, "phone_app")
		app.SetAttr(attr, value)
		if app.IsEmpty(appKindInfo) {
			t.Errorf("an app with %s is empty", attr)
		}
	}
	if !rule.NewRule(IOSAppKind, "phone_app").IsEmpty(appKindInfo) {
		t.Error("an app with no entry point is not empty")
	}
}
