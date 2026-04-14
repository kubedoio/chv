package uuidx

import (
	"testing"
)

func TestValidateSafeForPath(t *testing.T) {
	tests := []struct {
		name    string
		id      string
		wantErr bool
		errMsg  string
	}{
		// Valid UUIDs
		{
			name:    "valid UUID v4",
			id:      "550e8400-e29b-41d4-a716-446655440000",
			wantErr: false,
		},
		{
			name:    "valid UUID v1",
			id:      "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
			wantErr: false,
		},
		{
			name:    "valid UUID all zeros",
			id:      "00000000-0000-0000-0000-000000000000",
			wantErr: false,
		},
		{
			name:    "valid UUID all fs",
			id:      "ffffffff-ffff-ffff-ffff-ffffffffffff",
			wantErr: false,
		},
		// Path traversal attacks
		{
			name:    "path traversal unix",
			id:      "../../../etc/passwd",
			wantErr: true,
			errMsg:  "path separator",
		},
		{
			name:    "path traversal windows",
			id:      `..\..\windows\system32`,
			wantErr: true,
			errMsg:  "path separator",
		},
		{
			name:    "path traversal mixed",
			id:      `..\/../etc/passwd`,
			wantErr: true,
			errMsg:  "path separator",
		},
		{
			name:    "path traversal with valid prefix",
			id:      "550e8400-e29b-41d4-a716-446655440000/../../../etc",
			wantErr: true,
			errMsg:  "path separator",
		},
		{
			name:    "path traversal double dot only",
			id:      "..",
			wantErr: true,
			errMsg:  "path traversal",
		},
		{
			name:    "path traversal embedded",
			id:      "foo..bar",
			wantErr: true,
			errMsg:  "path traversal",
		},
		// Path separators
		{
			name:    "forward slash",
			id:      "test/file.txt",
			wantErr: true,
			errMsg:  "path separator",
		},
		{
			name:    "backslash",
			id:      `test\file.txt`,
			wantErr: true,
			errMsg:  "path separator",
		},
		{
			name:    "multiple slashes",
			id:      "/var/lib/chv/test",
			wantErr: true,
			errMsg:  "path separator",
		},
		// Invalid UUID formats
		{
			name:    "empty string",
			id:      "",
			wantErr: true,
			errMsg:  "valid UUID",
		},
		{
			name:    "too short",
			id:      "550e8400",
			wantErr: true,
			errMsg:  "valid UUID",
		},
		{
			name:    "too long",
			id:      "550e8400-e29b-41d4-a716-446655440000-extra",
			wantErr: true,
			errMsg:  "valid UUID",
		},
		{
			name:    "missing hyphens (still valid)",
			id:      "550e8400e29b41d4a716446655440000",
			wantErr: false, // google/uuid accepts this format
		},
		{
			name:    "invalid characters",
			id:      "550e8400-e29b-41d4-a716-44665544000g",
			wantErr: true,
			errMsg:  "valid UUID",
		},
		{
			name:    "URL encoded",
			id:      "550e8400%2De29b%2D41d4%2Da716%2D446655440000",
			wantErr: true,
			errMsg:  "valid UUID",
		},
		// Edge cases
		{
			name:    "single dot",
			id:      ".",
			wantErr: true,
			errMsg:  "valid UUID",
		},
		{
			name:    "single dot in middle",
			id:      "550e8400-e29b-41d4-a716-4466554400.0",
			wantErr: true,
			errMsg:  "valid UUID",
		},
		{
			name:    "null byte injection",
			id:      "550e8400-e29b-41d4-a716-446655440000\x00",
			wantErr: true,
			errMsg:  "valid UUID",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := ValidateSafeForPath(tt.id)
			if tt.wantErr {
				if err == nil {
					t.Errorf("ValidateSafeForPath(%q) expected error containing %q, got nil", tt.id, tt.errMsg)
					return
				}
				if !contains(err.Error(), tt.errMsg) {
					t.Errorf("ValidateSafeForPath(%q) error = %q, want error containing %q", tt.id, err.Error(), tt.errMsg)
				}
			} else {
				if err != nil {
					t.Errorf("ValidateSafeForPath(%q) expected no error, got %q", tt.id, err.Error())
				}
			}
		})
	}
}

func contains(s, substr string) bool {
	return len(substr) <= len(s) && (s == substr || len(s) > 0 && containsHelper(s, substr))
}

func containsHelper(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}

func TestIsValid(t *testing.T) {
	tests := []struct {
		name string
		s    string
		want bool
	}{
		{
			name: "valid UUID",
			s:    "550e8400-e29b-41d4-a716-446655440000",
			want: true,
		},
		{
			name: "invalid UUID",
			s:    "not-a-uuid",
			want: false,
		},
		{
			name: "path traversal",
			s:    "../../../etc/passwd",
			want: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := IsValid(tt.s)
			if got != tt.want {
				t.Errorf("IsValid(%q) = %v, want %v", tt.s, got, tt.want)
			}
		})
	}
}
