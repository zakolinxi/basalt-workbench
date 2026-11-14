//go:build !windows

package secrets

// Service matches the desktop workbench keyring service.
const Service = "com.basalt.workbench"

// ReadProviderKey is a no-op on platforms without the Windows credential
// store. Readers should fall back to environment variables or config files.
func ReadProviderKey(providerID string) (string, bool) {
	return "", false
}
