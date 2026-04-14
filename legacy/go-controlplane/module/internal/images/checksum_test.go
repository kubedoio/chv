package images

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestParseChecksum(t *testing.T) {
	tests := []struct {
		name        string
		checksum    string
		wantHash    string
		wantErr     bool
		errContains string
	}{
		{
			name:     "valid sha256 format",
			checksum: "sha256:abc123def456",
			wantHash: "abc123def456",
			wantErr:  false,
		},
		{
			name:     "empty checksum",
			checksum: "",
			wantHash: "",
			wantErr:  false,
		},
		{
			name:        "missing colon separator",
			checksum:    "sha256abc123",
			wantErr:     true,
			errContains: "invalid checksum format",
		},
		{
			name:        "unsupported algorithm",
			checksum:    "md5:abc123",
			wantErr:     true,
			errContains: "unsupported checksum algorithm",
		},
		{
			name:     "valid sha256 with long hash",
			checksum: "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
			wantHash: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
			wantErr:  false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := ParseChecksum(tt.checksum)
			if (err != nil) != tt.wantErr {
				t.Errorf("ParseChecksum() error = %v, wantErr %v", err, tt.wantErr)
				return
			}
			if tt.errContains != "" && err != nil && !strings.Contains(err.Error(), tt.errContains) {
				t.Errorf("ParseChecksum() error = %v, should contain %v", err, tt.errContains)
			}
			if got != tt.wantHash {
				t.Errorf("ParseChecksum() = %v, want %v", got, tt.wantHash)
			}
		})
	}
}

func TestCalculateSHA256(t *testing.T) {
	// Create temp directory for test files
	tmpDir := t.TempDir()

	tests := []struct {
		name         string
		content      string
		wantHash     string
		wantErr      bool
		errContains  string
		fileNotExist bool
	}{
		{
			name:     "empty file",
			content:  "",
			wantHash: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
			wantErr:  false,
		},
		{
			name:     "hello world",
			content:  "hello world",
			wantHash: "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
			wantErr:  false,
		},
		{
			name:     "test content",
			content:  "test content for checksum validation",
			wantHash: "b873ee26f3d17e038e023b4a4a9c9e3379ecc018171760b986abdbc011e17746",
			wantErr:  false,
		},
		{
			name:         "file not found",
			fileNotExist: true,
			wantErr:      true,
			errContains:  "failed to open file",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var path string
			if tt.fileNotExist {
				path = filepath.Join(tmpDir, "nonexistent.txt")
			} else {
				path = filepath.Join(tmpDir, tt.name+".txt")
				if err := os.WriteFile(path, []byte(tt.content), 0644); err != nil {
					t.Fatalf("failed to create test file: %v", err)
				}
			}

			got, err := CalculateSHA256(path)
			if (err != nil) != tt.wantErr {
				t.Errorf("CalculateSHA256() error = %v, wantErr %v", err, tt.wantErr)
				return
			}
			if tt.errContains != "" && err != nil && !strings.Contains(err.Error(), tt.errContains) {
				t.Errorf("CalculateSHA256() error = %v, should contain %v", err, tt.errContains)
			}
			if got != tt.wantHash {
				t.Errorf("CalculateSHA256() = %v, want %v", got, tt.wantHash)
			}
		})
	}
}

func TestValidateChecksum(t *testing.T) {
	// Create temp directory for test files
	tmpDir := t.TempDir()

	// Create a test file with known content
	testContent := "test content for checksum validation"
	testFile := filepath.Join(tmpDir, "test.txt")
	if err := os.WriteFile(testFile, []byte(testContent), 0644); err != nil {
		t.Fatalf("failed to create test file: %v", err)
	}

	// Known SHA256 hash for the test content (calculated with sha256sum)
	expectedHash := "b873ee26f3d17e038e023b4a4a9c9e3379ecc018171760b986abdbc011e17746"

	tests := []struct {
		name        string
		filePath    string
		checksum    string
		wantErr     bool
		errContains string
	}{
		{
			name:     "valid checksum with prefix",
			filePath: testFile,
			checksum: "sha256:" + expectedHash,
			wantErr:  false,
		},
		{
			name:     "empty checksum - no validation",
			filePath: testFile,
			checksum: "",
			wantErr:  false,
		},
		{
			name:        "checksum mismatch",
			filePath:    testFile,
			checksum:    "sha256:0000000000000000000000000000000000000000000000000000000000000000",
			wantErr:     true,
			errContains: "checksum mismatch",
		},
		{
			name:        "invalid checksum format",
			filePath:    testFile,
			checksum:    "invalidformat",
			wantErr:     true,
			errContains: "invalid checksum format",
		},
		{
			name:        "unsupported algorithm",
			filePath:    testFile,
			checksum:    "md5:" + expectedHash,
			wantErr:     true,
			errContains: "unsupported checksum algorithm",
		},
		{
			name:        "file not found",
			filePath:    filepath.Join(tmpDir, "nonexistent.txt"),
			checksum:    "sha256:" + expectedHash,
			wantErr:     true,
			errContains: "failed to open file",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := ValidateChecksum(tt.filePath, tt.checksum)
			if (err != nil) != tt.wantErr {
				t.Errorf("ValidateChecksum() error = %v, wantErr %v", err, tt.wantErr)
				return
			}
			if tt.errContains != "" && err != nil && !strings.Contains(err.Error(), tt.errContains) {
				t.Errorf("ValidateChecksum() error = %v, should contain %v", err, tt.errContains)
			}
		})
	}
}

func TestHashReader(t *testing.T) {
	tests := []struct {
		name     string
		content  string
		wantHash string
	}{
		{
			name:     "empty content",
			content:  "",
			wantHash: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
		},
		{
			name:     "hello world",
			content:  "hello world",
			wantHash: "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// Create a reader with the test content
			reader := bytes.NewReader([]byte(tt.content))
			hashReader := NewHashReader(reader)

			// Read all content
			buf := make([]byte, 1024)
			totalRead := 0
			for {
				n, err := hashReader.Read(buf)
				totalRead += n
				if err != nil {
					break
				}
			}

			// Verify content was read
			if totalRead != len(tt.content) {
				t.Errorf("HashReader read %d bytes, want %d", totalRead, len(tt.content))
			}

			// Get the hash
			got := hashReader.Sum()
			if got != tt.wantHash {
				t.Errorf("HashReader.Sum() = %v, want %v", got, tt.wantHash)
			}
		})
	}
}

func TestHashReader_MultipleReads(t *testing.T) {
	content := "hello world test content"
	reader := bytes.NewReader([]byte(content))
	hashReader := NewHashReader(reader)

	// Read in small chunks
	buf := make([]byte, 5)
	var allRead []byte
	for {
		n, err := hashReader.Read(buf)
		if n > 0 {
			allRead = append(allRead, buf[:n]...)
		}
		if err != nil {
			break
		}
	}

	// Verify all content was read
	if string(allRead) != content {
		t.Errorf("HashReader read %q, want %q", string(allRead), content)
	}

	// Verify hash is correct
	got := hashReader.Sum()
	if got == "" {
		t.Error("HashReader.Sum() returned empty string")
	}
}
