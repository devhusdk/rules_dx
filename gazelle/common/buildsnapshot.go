package common

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"sync"
)

// BuildSnapshot remembers the on-disk state of BUILD files before gazelle may
// write them, so a failed run can put every file back byte for byte.
type BuildSnapshot struct {
	mu    sync.Mutex
	files map[string]snapshotEntry
}

type snapshotEntry struct {
	data   []byte
	exists bool
	mode   os.FileMode
}

// Reset forgets all recorded files.
func (s *BuildSnapshot) Reset() {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.files = nil
}

// Record stores the current state of path once; later calls for the same path
// keep the first state.
func (s *BuildSnapshot) Record(path string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.files == nil {
		s.files = map[string]snapshotEntry{}
	}
	if _, ok := s.files[path]; ok {
		return
	}
	info, err := os.Stat(path)
	if err != nil || info.IsDir() {
		s.files[path] = snapshotEntry{}
		return
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return
	}
	s.files[path] = snapshotEntry{data: data, exists: true, mode: info.Mode().Perm()}
}

// RecordDir records every candidate BUILD file name in dir.
func (s *BuildSnapshot) RecordDir(dir string, names []string) {
	for _, name := range names {
		s.Record(filepath.Join(dir, name))
	}
}

// Restore rewrites changed files and removes files that did not exist when
// they were recorded. It is idempotent and returns every error joined.
func (s *BuildSnapshot) Restore() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	paths := make([]string, 0, len(s.files))
	for p := range s.files {
		paths = append(paths, p)
	}
	sort.Strings(paths)
	var errs []error
	for _, p := range paths {
		e := s.files[p]
		if !e.exists {
			if err := os.Remove(p); err != nil && !errors.Is(err, os.ErrNotExist) {
				errs = append(errs, fmt.Errorf("restore %s: %w", p, err))
			}
			continue
		}
		if cur, err := os.ReadFile(p); err == nil && string(cur) == string(e.data) {
			continue
		}
		if err := os.WriteFile(p, e.data, e.mode); err != nil {
			errs = append(errs, fmt.Errorf("restore %s: %w", p, err))
		}
	}
	return errors.Join(errs...)
}
