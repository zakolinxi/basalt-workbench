//go:build windows

// Package secrets reads provider API keys from the Windows Credential Manager.
// The desktop workbench stores them through keyring-rs, whose Windows target
// name is "<account>.<service>".
package secrets

import (
	"encoding/binary"
	"strings"
	"unicode/utf16"
	"unsafe"

	"golang.org/x/sys/windows"
)

const Service = "com.basalt.workbench"

type credential struct {
	Flags              uint32
	Type               uint32
	TargetName         *uint16
	Comment            *uint16
	LastWritten        windows.Filetime
	CredentialBlobSize uint32
	CredentialBlob     *byte
	Persist            uint32
	AttributeCount     uint32
	Attributes         uintptr
	TargetAlias        *uint16
	UserName           *uint16
}

const credTypeGeneric = 1

var (
	advapi32     = windows.NewLazySystemDLL("advapi32.dll")
	procCredRead = advapi32.NewProc("CredReadW")
	procCredFree = advapi32.NewProc("CredFree")
)

// ReadProviderKey returns the stored API key for a provider id.
func ReadProviderKey(providerID string) (string, bool) {
	if providerID == "" {
		return "", false
	}
	target, err := windows.UTF16PtrFromString(providerID + "." + Service)
	if err != nil {
		return "", false
	}
	var cred *credential
	r, _, _ := procCredRead.Call(
		uintptr(unsafe.Pointer(target)),
		uintptr(credTypeGeneric),
		0,
		uintptr(unsafe.Pointer(&cred)),
	)
	if r == 0 || cred == nil {
		return "", false
	}
	defer procCredFree.Call(uintptr(unsafe.Pointer(cred)))
	if cred.CredentialBlobSize == 0 || cred.CredentialBlob == nil {
		return "", false
	}
	blob := unsafe.Slice(cred.CredentialBlob, cred.CredentialBlobSize)
	key := decode(blob)
	if key == "" {
		return "", false
	}
	return key, true
}

// decode handles the UTF-16LE blobs written by keyring-rs as well as plain
// UTF-8 blobs written by older builds.
func decode(blob []byte) string {
	if len(blob) >= 2 && len(blob)%2 == 0 {
		units := make([]uint16, 0, len(blob)/2)
		for i := 0; i+1 < len(blob); i += 2 {
			units = append(units, binary.LittleEndian.Uint16(blob[i:]))
		}
		if s := strings.TrimRight(string(utf16.Decode(units)), "\x00"); printable(s) {
			return s
		}
	}
	return strings.TrimRight(string(blob), "\x00")
}

func printable(s string) bool {
	if s == "" {
		return false
	}
	for _, r := range s {
		if r < 0x20 || r == 0x7f {
			return false
		}
	}
	return true
}
