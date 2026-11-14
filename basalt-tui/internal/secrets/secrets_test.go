//go:build windows

package secrets

import (
	"encoding/binary"
	"testing"
	"unicode/utf16"
)

func utf16le(s string) []byte {
	units := utf16.Encode([]rune(s))
	out := make([]byte, len(units)*2)
	for i, u := range units {
		binary.LittleEndian.PutUint16(out[i*2:], u)
	}
	return out
}

func TestDecodeUTF16(t *testing.T) {
	key := "sk-abcdef0123456789"
	if got := decode(utf16le(key)); got != key {
		t.Fatalf("utf16 decode failed: %q", got)
	}
}

func TestDecodeUTF8(t *testing.T) {
	key := "sk-abcdef0123456789"
	if got := decode([]byte(key)); got != key {
		t.Fatalf("utf8 decode failed: %q", got)
	}
}

func TestDecodeTrailingNUL(t *testing.T) {
	blob := append(utf16le("sk-x"), 0, 0)
	if got := decode(blob); got != "sk-x" {
		t.Fatalf("trailing NUL not trimmed: %q", got)
	}
}

func TestReadUnknownProvider(t *testing.T) {
	if _, ok := ReadProviderKey("missing-provider-id-12345"); ok {
		t.Fatal("unexpected credential for a random id")
	}
}
