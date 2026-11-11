package common

import (
	"encoding/json"
	"os"
	"path/filepath"
)

// DubProject is the part of a dub.json that generation reads.
type DubProject struct {
	Name              string                     `json:"name"`
	TargetType        string                     `json:"targetType"`
	SourcePaths       []string                   `json:"sourcePaths"`
	ImportPaths       []string                   `json:"importPaths"`
	StringImportPaths []string                   `json:"stringImportPaths"`
	Dependencies      map[string]json.RawMessage `json:"dependencies"`
}

// ParseDubProject reads a dub.json.
func ParseDubProject(content []byte) (*DubProject, error) {
	var p DubProject
	if err := json.Unmarshal(StripBOM(content), &p); err != nil {
		return nil, err
	}
	return &p, nil
}

// ReadDubProject reads the dub.json in dir. It returns nil when there is none
// or when it does not parse.
func ReadDubProject(dir string) *DubProject {
	content, err := os.ReadFile(filepath.Join(dir, "dub.json"))
	if err != nil {
		return nil
	}
	p, err := ParseDubProject(content)
	if err != nil {
		return nil
	}
	return p
}

// Roots returns the import roots of the project: importPaths, else
// sourcePaths, else the source or src directory that exists in dir.
func (p *DubProject) Roots(dir string) []string {
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
