package d

import (
	"encoding/json"
	"os"
	"path/filepath"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

type dubProject struct {
	Name              string                     `json:"name"`
	TargetType        string                     `json:"targetType"`
	SourcePaths       []string                   `json:"sourcePaths"`
	ImportPaths       []string                   `json:"importPaths"`
	StringImportPaths []string                   `json:"stringImportPaths"`
	Dependencies      map[string]json.RawMessage `json:"dependencies"`
}

func parseDubProject(content []byte) (*dubProject, error) {
	var p dubProject
	if err := json.Unmarshal(common.StripBOM(content), &p); err != nil {
		return nil, err
	}
	return &p, nil
}

func readDubProject(dir string) *dubProject {
	content, err := os.ReadFile(filepath.Join(dir, common.ManifestName("dub")))
	if err != nil {
		return nil
	}
	p, err := parseDubProject(content)
	if err != nil {
		return nil
	}
	return p
}

func (p *dubProject) Roots(dir string) []string {
	switch {
	case len(p.ImportPaths) > 0:
		return p.ImportPaths
	case len(p.SourcePaths) > 0:
		return p.SourcePaths
	}
	for _, d := range []string{"source", "src"} {
		if info, err := os.Stat(filepath.Join(dir, d)); err == nil && info.IsDir() {
			return []string{d}
		}
	}
	return nil
}
