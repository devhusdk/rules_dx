package common

import (
	"sort"
	"sync"
)

// PackageRef names the Bazel package a failure applies to. The zero value
// means the package is unknown; InPackage("") is the root package.
type PackageRef struct {
	Rel   string
	Known bool
}

// UnknownPackage marks a run-level failure with no single package.
var UnknownPackage = PackageRef{}

// InPackage returns a reference to the package at rel ("" is the root).
func InPackage(rel string) PackageRef { return PackageRef{Rel: rel, Known: true} }

// GenerationError is one failure a language recorded during generation.
type GenerationError struct {
	Language string
	Package  PackageRef
	Message  string
}

// ErrorSink collects generation errors from every language so the last
// language (dispatch) can report all of them once and pick the exit code.
type ErrorSink struct {
	mu      sync.Mutex
	entries []GenerationError
}

// Errors is the sink every language writes to in AfterResolvingDeps.
var Errors = &ErrorSink{}

// Add records entries.
func (s *ErrorSink) Add(entries ...GenerationError) {
	if len(entries) == 0 {
		return
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	s.entries = append(s.entries, entries...)
}

// Reset drops every recorded error.
func (s *ErrorSink) Reset() {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.entries = nil
}

// Take returns the recorded errors sorted by (Language, Package, Message), unknown packages first,
// without duplicates, and empties the sink.
func (s *ErrorSink) Take() []GenerationError {
	s.mu.Lock()
	entries := s.entries
	s.entries = nil
	s.mu.Unlock()
	return SortErrors(entries)
}

// SortErrors orders entries by (Language, Package, Message), unknown packages first,, drops
// duplicates, and returns nil when none remain. It reuses entries' storage.
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

// Messages returns the Message of each entry, in order.
func Messages(entries []GenerationError) []string {
	msgs := make([]string, 0, len(entries))
	for _, e := range entries {
		msgs = append(msgs, e.Message)
	}
	return msgs
}
