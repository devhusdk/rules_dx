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
	link   bool
	dir    bool
	target string
}

// Reset forgets all recorded files.
func (s *BuildSnapshot) Reset() {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.files = nil
}

// Record stores the current state of path once; later calls for the same path
// keep the first state. A directory is left alone. It fails if the file cannot be inspected or read.
func (s *BuildSnapshot) Record(path string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.files == nil {
		s.files = map[string]snapshotEntry{}
	}
	if _, ok := s.files[path]; ok {
		return nil
	}
	info, err := os.Stat(path)
	if errors.Is(err, os.ErrNotExist) || (err == nil && info.IsDir()) {
		if target, lerr := os.Readlink(path); lerr == nil {
			s.files[path] = snapshotEntry{link: true, target: target}
			return nil
		}
		if err == nil {
			s.files[path] = snapshotEntry{dir: true}
			return nil
		}
		s.files[path] = snapshotEntry{}
		return nil
	}
	if err != nil {
		return fmt.Errorf("snapshot %s: %w", path, err)
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return fmt.Errorf("snapshot %s: %w", path, err)
	}
	s.files[path] = snapshotEntry{data: data, exists: true, mode: info.Mode().Perm()}
	return nil
}

// RecordDir records every candidate BUILD file name in dir and returns every
// error joined.
func (s *BuildSnapshot) RecordDir(dir string, names []string) error {
	var errs []error
	for _, name := range names {
		if err := s.Record(filepath.Join(dir, name)); err != nil {
			errs = append(errs, err)
		}
	}
	return errors.Join(errs...)
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
		if e.link {
			if err := restoreLink(p, e.target); err != nil {
				errs = append(errs, fmt.Errorf("restore %s: %w", p, err))
			}
			continue
		}
		if e.dir {
			continue
		}
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

// restoreLink keeps a link that still points at target and otherwise replaces
// whatever is at path with a link to target.
func restoreLink(path, target string) error {
	if cur, err := os.Readlink(path); err == nil && cur == target {
		return nil
	}
	if err := os.Remove(path); err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	return os.Symlink(target, path)
}
