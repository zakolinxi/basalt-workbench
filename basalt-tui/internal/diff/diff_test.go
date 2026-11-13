package diff

import "testing"

func TestLinesBasic(t *testing.T) {
	ops := Lines("a\nb\nc\n", "a\nB\nc\n")
	added, removed := Stats(ops)
	if added != 1 || removed != 1 {
		t.Fatalf("expected +1 -1, got +%d -%d", added, removed)
	}
	out := Unified("x.txt", ops, 1)
	if out == "" {
		t.Fatal("expected a unified diff")
	}
}

func TestNoChange(t *testing.T) {
	ops := Lines("same\n", "same\n")
	added, removed := Stats(ops)
	if added != 0 || removed != 0 {
		t.Fatalf("expected no changes, got +%d -%d", added, removed)
	}
	if Unified("x", ops, 3) != "" {
		t.Fatal("expected empty diff")
	}
}

func TestInsertAtEnd(t *testing.T) {
	ops := Lines("a\n", "a\nb\n")
	if added, removed := Stats(ops); added != 1 || removed != 0 {
		t.Fatalf("expected +1 -0, got +%d -%d", added, removed)
	}
}

func TestCRLFNormalised(t *testing.T) {
	ops := Lines("a\r\nb\r\n", "a\nb\n")
	if added, removed := Stats(ops); added != 0 || removed != 0 {
		t.Fatalf("CRLF should not count as a change, got +%d -%d", added, removed)
	}
}
