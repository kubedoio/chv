// Package validation provides input validation utilities for the CHV platform.
package validation

import (
	"fmt"
	"path/filepath"
	"regexp"
	"strings"
)

// ValidIDPattern matches valid identifiers (VM IDs, Image IDs, Volume IDs)
// Allows: alphanumeric, hyphens, underscores, dots; must start with alphanumeric
var ValidIDPattern = regexp.MustCompile(`^[a-zA-Z0-9][a-zA-Z0-9_.-]*$`)

// MaxIDLength is the maximum allowed length for identifiers
const MaxIDLength = 64

// ValidateID validates that an identifier is safe to use in file paths.
// It prevents path traversal attacks by rejecting paths with separators,
// parent directory references, and other unsafe characters.
func ValidateID(id string) error {
	if id == "" {
		return fmt.Errorf("ID cannot be empty")
	}
	if len(id) > MaxIDLength {
		return fmt.Errorf("ID exceeds maximum length of %d characters", MaxIDLength)
	}
	// Check for path traversal attempts
	if filepath.IsAbs(id) {
		return fmt.Errorf("ID cannot be an absolute path")
	}
	// Check for special path components
	if id == "." || id == ".." {
		return fmt.Errorf("ID cannot be '.' or '..'")
	}
	// Check for any path separators or traversal sequences
	clean := filepath.Clean(id)
	if clean != id {
		return fmt.Errorf("ID contains invalid characters or path traversal")
	}
	// Check that it doesn't contain any path separators
	if strings.ContainsAny(id, `/\`) {
		return fmt.Errorf("ID cannot contain path separators")
	}
	// Validate against allowed pattern
	if !ValidIDPattern.MatchString(id) {
		return fmt.Errorf("ID must start with alphanumeric and contain only alphanumeric, hyphens, and underscores")
	}
	return nil
}

// ValidateIDSlice validates multiple IDs at once.
func ValidateIDSlice(ids ...string) error {
	for _, id := range ids {
		if err := ValidateID(id); err != nil {
			return err
		}
	}
	return nil
}

// SanitizePathComponent validates that a string is safe to use as a single
// path component (e.g., filename) and returns it if valid.
func SanitizePathComponent(name string) (string, error) {
	if err := ValidateID(name); err != nil {
		return "", err
	}
	return name, nil
}

// BuildSafePath builds a path by joining base directory with validated components.
// It ensures that the resulting path does not escape the base directory.
func BuildSafePath(base string, components ...string) (string, error) {
	// Validate all components
	for _, comp := range components {
		if err := ValidateID(comp); err != nil {
			return "", fmt.Errorf("invalid path component %q: %w", comp, err)
		}
	}
	
	// Join the path
	result := filepath.Join(append([]string{base}, components...)...)
	
	// Ensure the result is within the base directory
	// (defense in depth - ValidateID should already prevent escaping)
	absBase, err := filepath.Abs(base)
	if err != nil {
		return "", fmt.Errorf("failed to get absolute path for base: %w", err)
	}
	absResult, err := filepath.Abs(result)
	if err != nil {
		return "", fmt.Errorf("failed to get absolute path for result: %w", err)
	}
	
	// Check that result starts with base + separator (or equals base if no components)
	if !strings.HasPrefix(absResult, absBase+string(filepath.Separator)) && absResult != absBase {
		return "", fmt.Errorf("path escapes base directory")
	}
	
	return result, nil
}
