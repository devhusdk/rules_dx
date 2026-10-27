package common

import (
	"sort"
	"sync"
)

// GenerationError is one failure a language recorded during generation.
// Package is the Bazel package it applies to, or "" when unknown.
type GenerationError struct {
	Language string
	Package  string
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

// Add records msgs for language and pkg.
func (s *ErrorSink) Add(language, pkg string, msgs ...string) {
	if len(msgs) == 0 {
		return
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	for _, msg := range msgs {
		s.entries = append(s.entries, GenerationError{Language: language, Package: pkg, Message: msg})
	}
}

// Reset drops every recorded error.
func (s *ErrorSink) Reset() {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.entries = nil
}

// Take returns the recorded errors sorted by (Language, Package, Message)
// without duplicates, and empties the sink.
func (s *ErrorSink) Take() []GenerationError {
	s.mu.Lock()
	entries := s.entries
	s.entries = nil
	s.mu.Unlock()
	sort.Slice(entries, func(i, j int) bool {
		a, b := entries[i], entries[j]
		if a.Language != b.Language {
			return a.Language < b.Language
		}
		if a.Package != b.Package {
			return a.Package < b.Package
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
