// Package uuidx provides UUID utilities for CHV.
package uuidx

import (
	"errors"
	"strings"

	"github.com/google/uuid"
)

// New generates a new random UUID.
func New() uuid.UUID {
	return uuid.Must(uuid.NewRandom())
}

// NewString generates a new random UUID string.
func NewString() string {
	return New().String()
}

// Parse parses a UUID from string.
func Parse(s string) (uuid.UUID, error) {
	return uuid.Parse(s)
}

// MustParse parses a UUID from string, panicking on error.
func MustParse(s string) uuid.UUID {
	return uuid.MustParse(s)
}

// IsValid checks if a string is a valid UUID.
func IsValid(s string) bool {
	_, err := uuid.Parse(s)
	return err == nil
}

// ValidateSafeForPath validates that a VM ID is safe to use in file paths.
// It checks that the ID:
//   - Is a valid UUID (36 chars, standard format)
//   - Does not contain path separators (/ or \)
//   - Does not contain path traversal sequences (..)
//
// This prevents path traversal attacks where malicious IDs like
// "../../../etc/passwd" could escape intended directories.
func ValidateSafeForPath(id string) error {
	// Check for path separators
	if strings.Contains(id, "/") {
		return errors.New("VM ID contains path separator '/'")
	}
	if strings.Contains(id, "\\") {
		return errors.New("VM ID contains path separator '\\'")
	}

	// Check for path traversal sequences
	if strings.Contains(id, "..") {
		return errors.New("VM ID contains path traversal sequence '..'")
	}

	// Validate UUID format
	if _, err := uuid.Parse(id); err != nil {
		return errors.New("VM ID is not a valid UUID")
	}

	return nil
}
