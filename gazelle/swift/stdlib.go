package swift

var stdlib = map[string]struct{}{
	"Swift": {}, "_Concurrency": {}, "Foundation": {}, "FoundationEssentials": {},
	"FoundationNetworking": {}, "FoundationXML": {}, "Dispatch": {}, "Glibc": {},
	"Musl": {}, "Darwin": {}, "Observation": {}, "RegexBuilder": {}, "Synchronization": {},
	"XCTest": {}, "Testing": {}, "Cxx": {}, "CoreFoundation": {},
	"AppKit": {}, "SwiftUI": {}, "UIKit": {},
}

func IsStdLib(name string) bool {
	_, ok := stdlib[name]
	return ok
}
