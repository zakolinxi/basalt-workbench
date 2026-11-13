// Package tools implements the workspace tools the agent can call.
package tools

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
)

// Env is the execution environment shared by all tools.
type Env struct {
	Root      string
	SessionID string
	DataDir   string

	mu        sync.Mutex
	snapshots []Snapshot
}

// Snapshot stores the previous content of a file so /undo can restore it.
type Snapshot struct {
	Path    string
	Content string
	Existed bool
}

// NewEnv creates an environment rooted at the workspace.
func NewEnv(root, sessionID, dataDir string) *Env {
	abs, err := filepath.Abs(root)
	if err == nil {
		root = abs
	}
	return &Env{Root: root, SessionID: sessionID, DataDir: dataDir}
}

// Resolve turns a user/model supplied path into an absolute path inside Root.
func (e *Env) Resolve(p string) (string, error) {
	p = strings.TrimSpace(p)
	p = strings.TrimPrefix(p, "@")
	if p == "" || p == "." {
		return e.Root, nil
	}
	p = strings.ReplaceAll(p, "\\", "/")
	var abs string
	if filepath.IsAbs(p) || (len(p) > 1 && p[1] == ':') {
		abs = filepath.Clean(p)
	} else {
		abs = filepath.Clean(filepath.Join(e.Root, p))
	}
	rel, err := filepath.Rel(e.Root, abs)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return "", fmt.Errorf("路径越界：%s 不在工作区内", p)
	}
	return abs, nil
}

// Rel renders a path relative to the workspace with forward slashes.
func (e *Env) Rel(abs string) string {
	rel, err := filepath.Rel(e.Root, abs)
	if err != nil {
		return abs
	}
	return filepath.ToSlash(rel)
}

// Remember snapshots a file's current content before a mutation.
func (e *Env) Remember(abs string) {
	raw, err := os.ReadFile(abs)
	e.mu.Lock()
	defer e.mu.Unlock()
	e.snapshots = append(e.snapshots, Snapshot{
		Path:    abs,
		Content: string(raw),
		Existed: err == nil,
	})
}

// PopSnapshot removes and returns the most recent snapshot.
func (e *Env) PopSnapshot() (Snapshot, bool) {
	e.mu.Lock()
	defer e.mu.Unlock()
	if len(e.snapshots) == 0 {
		return Snapshot{}, false
	}
	s := e.snapshots[len(e.snapshots)-1]
	e.snapshots = e.snapshots[:len(e.snapshots)-1]
	return s, true
}

// SnapshotCount reports how many undoable mutations are pending.
func (e *Env) SnapshotCount() int {
	e.mu.Lock()
	defer e.mu.Unlock()
	return len(e.snapshots)
}
