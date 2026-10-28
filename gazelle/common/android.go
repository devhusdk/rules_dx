package common

import (
	"bytes"
	"encoding/xml"
	"errors"
	"fmt"
	"io"
	"strconv"
	"strings"
)

// AndroidNS is the namespace URI of android: manifest attributes.
const AndroidNS = "http://schemas.android.com/apk/res/android"

// AndroidManifest holds the facts generation reads from an AndroidManifest.xml.
// MinSDK and TargetSDK are 0 when the manifest does not declare them.
type AndroidManifest struct {
	Package     string
	MinSDK      int
	TargetSDK   int
	Permissions []string
}

// ParseAndroidManifest reads the package, SDK levels and permissions of a manifest.
func ParseAndroidManifest(content []byte) (AndroidManifest, error) {
	var m AndroidManifest
	dec := xml.NewDecoder(bytes.NewReader(content))
	depth := 0
	sawRoot := false
	sawSDK := false
	for {
		tok, err := dec.Token()
		if errors.Is(err, io.EOF) {
			break
		}
		if err != nil {
			return AndroidManifest{}, fmt.Errorf("malformed manifest: %v", err)
		}
		switch t := tok.(type) {
		case xml.StartElement:
			depth++
			if depth == 1 {
				if t.Name.Local != "manifest" {
					return AndroidManifest{}, fmt.Errorf("root element is <%s>, want <manifest>", t.Name.Local)
				}
				sawRoot = true
				for _, a := range t.Attr {
					if a.Name.Space == "" && a.Name.Local == "package" {
						m.Package = strings.TrimSpace(a.Value)
					}
				}
				continue
			}
			if depth != 2 {
				continue
			}
			switch t.Name.Local {
			case "uses-sdk":
				if sawSDK {
					return AndroidManifest{}, errors.New("more than one <uses-sdk>")
				}
				sawSDK = true
				for _, a := range t.Attr {
					if a.Name.Space != AndroidNS {
						continue
					}
					var dst *int
					switch a.Name.Local {
					case "minSdkVersion":
						dst = &m.MinSDK
					case "targetSdkVersion":
						dst = &m.TargetSDK
					default:
						continue
					}
					n, err := strconv.Atoi(strings.TrimSpace(a.Value))
					if err != nil {
						return AndroidManifest{}, fmt.Errorf("android:%s %q is not an integer", a.Name.Local, a.Value)
					}
					*dst = n
				}
			case "uses-permission":
				for _, a := range t.Attr {
					if a.Name.Space == AndroidNS && a.Name.Local == "name" {
						m.Permissions = append(m.Permissions, a.Value)
					}
				}
			}
		case xml.EndElement:
			depth--
		}
	}
	if !sawRoot {
		return AndroidManifest{}, errors.New("no <manifest> root element")
	}
	return m, nil
}

var ndkAndroidLibHeaders = map[string]bool{
	"native_activity":   true,
	"looper":            true,
	"input":             true,
	"asset_manager":     true,
	"asset_manager_jni": true,
	"configuration":     true,
	"sensor":            true,
	"native_window":     true,
}

// NDKLib reports whether header is an NDK android/ header and the system library it links, if any.
func NDKLib(header string) (string, bool) {
	rest, ok := strings.CutPrefix(header, "android/")
	if !ok || rest == "" {
		return "", false
	}
	base := strings.TrimSuffix(rest, ".h")
	if base == "log" {
		return "log", true
	}
	if base != rest && ndkAndroidLibHeaders[base] {
		return "android", true
	}
	return "", true
}

var androidSDKPackages = []string{"android", "dalvik"}

// androidMavenPackages ship as Maven artifacts, not in android.jar.
var androidMavenPackages = []string{"android.support", "android.arch"}

// androidBundledPackages ship in android.jar next to the android packages.
var androidBundledPackages = []string{"org.json", "org.xmlpull.v1"}

// IsAndroidSDKPlatform reports whether an import is an android.jar package
// that selects an Android library kind.
func IsAndroidSDKPlatform(spec string) bool {
	return InPackages(spec, androidSDKPackages) && !InPackages(spec, androidMavenPackages)
}

// IsAndroidPlatformImport reports whether an import resolves against android.jar.
func IsAndroidPlatformImport(spec string) bool {
	return IsAndroidSDKPlatform(spec) || InPackages(spec, androidBundledPackages)
}

// AndroidVariant returns the KindVariant generating kind for sources that
// import the Android SDK.
func AndroidVariant(kind string) *KindVariant {
	return &KindVariant{
		Kind:         kind,
		DefsPackage:  "android/rules:defs.bzl",
		Selects:      IsAndroidSDKPlatform,
		IsPlatform:   IsAndroidPlatformImport,
		NeedsPackage: "Android sources need a package declaration to set custom_package",
		Attrs: func(pkg string) []KindAttr {
			return []KindAttr{{"custom_package", pkg}, {"tags", []string{"manual"}}}
		},
	}
}
