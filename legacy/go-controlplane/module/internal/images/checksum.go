package images

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"hash"
	"io"
	"os"
	"strings"
)

// ParseChecksum parses the "sha256:abc123..." format
func ParseChecksum(checksum string) (string, error) {
	if checksum == "" {
		return "", nil // No checksum provided
	}

	parts := strings.SplitN(checksum, ":", 2)
	if len(parts) != 2 {
		return "", fmt.Errorf("invalid checksum format: expected sha256:hash")
	}

	if parts[0] != "sha256" {
		return "", fmt.Errorf("unsupported checksum algorithm: %s", parts[0])
	}

	return parts[1], nil
}

// CalculateSHA256 calculates the SHA256 hash of a file
func CalculateSHA256(path string) (string, error) {
	file, err := os.Open(path)
	if err != nil {
		return "", fmt.Errorf("failed to open file: %w", err)
	}
	defer file.Close()

	hasher := sha256.New()
	if _, err := io.Copy(hasher, file); err != nil {
		return "", fmt.Errorf("failed to hash file: %w", err)
	}

	return hex.EncodeToString(hasher.Sum(nil)), nil
}

// ValidateChecksum validates a file against an expected SHA256 checksum
// Expected format: "sha256:abc123..." or just the hash
func ValidateChecksum(filePath, expectedChecksum string) error {
	if expectedChecksum == "" {
		return nil // No checksum to validate
	}

	hash, err := ParseChecksum(expectedChecksum)
	if err != nil {
		return err
	}

	actual, err := CalculateSHA256(filePath)
	if err != nil {
		return err
	}

	if !strings.EqualFold(actual, hash) {
		return fmt.Errorf("checksum mismatch: expected %s, got %s", hash, actual)
	}

	return nil
}

// HashReader wraps an io.Reader to calculate hash while reading
type HashReader struct {
	reader io.Reader
	hasher hash.Hash
}

func NewHashReader(r io.Reader) *HashReader {
	return &HashReader{
		reader: r,
		hasher: sha256.New(),
	}
}

func (h *HashReader) Read(p []byte) (n int, err error) {
	n, err = h.reader.Read(p)
	if n > 0 {
		h.hasher.Write(p[:n])
	}
	return n, err
}

func (h *HashReader) Sum() string {
	return hex.EncodeToString(h.hasher.Sum(nil))
}
