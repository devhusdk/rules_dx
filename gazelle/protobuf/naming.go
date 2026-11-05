package protobuf

import (
	"path"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

var SupportedExts = []string{".proto"}

const (
	LibraryKind = "proto_library"
	suffix      = "_proto"
	wellKnown   = "google/protobuf/"
)

// DirTargetName names the proto_library of a directory.
func DirTargetName(dir string) (string, error) {
	name, err := common.DirTargetName(dir)
	if err != nil {
		return "", err
	}
	return name + suffix, nil
}

// WellKnownLabel maps google/protobuf/<name>.proto to @protobuf//:<name>_proto.
func WellKnownLabel(imp string) (string, bool) {
	rest, ok := strings.CutPrefix(imp, wellKnown)
	if !ok || path.Dir(rest) != "." {
		return "", false
	}
	stem, ok := strings.CutSuffix(rest, ".proto")
	if !ok || stem == "" {
		return "", false
	}
	return "@protobuf//:" + stem + suffix, true
}
