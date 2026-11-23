package common

import (
	"sort"
	"strings"
	"sync"
)

type PackageRef struct {
	Rel   string
	Known bool
}

var UnknownPackage = PackageRef{}

func InPackage(rel string) PackageRef { return PackageRef{Rel: rel, Known: true} }

type GenerationError struct {
	Language string
	Package  PackageRef
	Message  string
	Kind     string
	Import   string
	Fix      Fix
}

const (
	KindUnresolvedImport = "unresolved_import"
	KindAmbiguousImport  = "ambiguous_import"
)

type Fix struct {
	File      string `json:"file"`
	Directive string `json:"directive"`
}

type ErrorSink struct {
	mu      sync.Mutex
	entries []GenerationError
}

var Errors = &ErrorSink{}

func (s *ErrorSink) Add(entries ...GenerationError) {
	if len(entries) == 0 {
		return
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	s.entries = append(s.entries, entries...)
}

func (s *ErrorSink) Reset() {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.entries = nil
}

func (s *ErrorSink) Take() []GenerationError {
	s.mu.Lock()
	entries := s.entries
	s.entries = nil
	s.mu.Unlock()
	return SortErrors(entries)
}

func SortErrors(entries []GenerationError) []GenerationError {
	sort.Slice(entries, func(i, j int) bool {
		a, b := entries[i], entries[j]
		if a.Language != b.Language {
			return a.Language < b.Language
		}
		if a.Package.Known != b.Package.Known {
			return !a.Package.Known
		}
		if a.Package.Rel != b.Package.Rel {
			return a.Package.Rel < b.Package.Rel
		}
		return a.Message < b.Message
	})
	out := entries[:0]
	for i, e := range entries {
		if i > 0 && e == entries[i-1] {
			continue
		}
		out = append(out, e)
	}
	if len(out) == 0 {
		return nil
	}
	return out
}

func Report(entries []GenerationError) []string {
	msgs := make([]string, 0, len(entries))
	for _, e := range entries {
		label := "//" + e.Package.Rel + ":"
		if !e.Package.Known || strings.Contains(e.Message, label) {
			msgs = append(msgs, e.Message)
			continue
		}
		msgs = append(msgs, label+" "+e.Message)
	}
	return msgs
}

func Messages(entries []GenerationError) []string {
	msgs := make([]string, 0, len(entries))
	for _, e := range entries {
		msgs = append(msgs, e.Message)
	}
	return msgs
}
