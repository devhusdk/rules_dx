package swift

import "testing"

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
