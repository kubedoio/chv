package validation

import (
	"path/filepath"
	"strings"
	"testing"
)

func TestValidateID(t *testing.T) {
	tests := []struct {
		name    string
		id      string
		wantErr bool
	}{
		{"valid simple", "vm-123", false},
		{"valid with underscore", "vm_test_1", false},
		{"valid alphanumeric", "abc123", false},
		{"valid uppercase", "VM-ABC-123", false},
		{"empty", "", true},
		{"dot", ".", true},
		{"dotdot", "..", true},
		{"absolute path", "/etc/passwd", true},
		{"path with slash", "vm/123", true},
		{"path with backslash", "vm\\123", true},
		{"traversal simple", "../etc/passwd", true},
		{"traversal embedded", "vm-../etc", true},
		{"traversal at end", "vm-123/../../../etc", true},
		{"null byte", "vm\x00test", true},
		{"too long", strings.Repeat("a", MaxIDLength+1), true},
		{"starts with hyphen", "-vm123", true},
		{"starts with underscore", "_vm123", true},
		{"special chars", "vm@123", true},
		{"space", "vm 123", true},
		{"tab", "vm\t123", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := ValidateID(tt.id)
			if (err != nil) != tt.wantErr {
				t.Errorf("ValidateID(%q) error = %v, wantErr %v", tt.id, err, tt.wantErr)
			}
		})
	}
}

func TestValidateIDSlice(t *testing.T) {
	tests := []struct {
		name    string
		ids     []string
		wantErr bool
	}{
		{"all valid", []string{"vm-1", "vm-2", "vm-3"}, false},
		{"one invalid", []string{"vm-1", "../etc", "vm-3"}, true},
		{"empty slice", []string{}, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := ValidateIDSlice(tt.ids...)
			if (err != nil) != tt.wantErr {
				t.Errorf("ValidateIDSlice() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestBuildSafePath(t *testing.T) {
	base := "/data/vms"
	
	tests := []struct {
		name       string
		components []string
		wantErr    bool
	}{
		{"single component", []string{"vm-123"}, false},
		{"multiple components", []string{"vm-123", "disk.raw"}, false},
		{"traversal attempt", []string{"..", "etc", "passwd"}, true},
		{"empty component", []string{""}, true},
		{"slash in component", []string{"vm/123"}, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := BuildSafePath(base, tt.components...)
			if (err != nil) != tt.wantErr {
				t.Errorf("BuildSafePath() error = %v, wantErr %v", err, tt.wantErr)
				return
			}
			if err == nil {
				// Verify result is under base
				if !strings.HasPrefix(result, base) {
					t.Errorf("BuildSafePath() result %q not under base %q", result, base)
				}
			}
		})
	}
}

func TestPathTraversalDefenses(t *testing.T) {
	// Test various path traversal attacks
	attacks := []string{
		"../../../etc/passwd",
		"..\\..\\..\\windows\\system32\\config\\sam",
		"vm-123/../../../etc/shadow",
		"vm-123/..",
		"vm-123/../..",
		"./../etc/passwd",
		"vm-123\x00/../../../etc/passwd", // null byte injection
	}

	for _, attack := range attacks {
		t.Run("attack_"+attack, func(t *testing.T) {
			err := ValidateID(attack)
			if err == nil {
				t.Errorf("ValidateID(%q) should have rejected path traversal", attack)
			}
		})
	}
}

func TestBuildSafePathNoEscape(t *testing.T) {
	// Ensure BuildSafePath cannot be tricked into escaping base directory
	base := t.TempDir()
	
	// Try to escape using multiple components
	_, err := BuildSafePath(base, "vm-123", "..", "..", "etc", "passwd")
	if err == nil {
		t.Error("BuildSafePath should reject traversal in components")
	}
	
	// Ensure the result is always under base even with valid components
	result, err := BuildSafePath(base, "vm-123", "disk.raw")
	if err != nil {
		t.Fatalf("BuildSafePath failed: %v", err)
	}
	
	// Verify it's actually under base
	rel, err := filepath.Rel(base, result)
	if err != nil {
		t.Fatalf("filepath.Rel failed: %v", err)
	}
	
	if strings.HasPrefix(rel, "..") {
		t.Errorf("Path escaped base: %q", result)
	}
}
