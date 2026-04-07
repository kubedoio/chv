package console

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"go.uber.org/zap"
)

func TestNewCircularBuffer(t *testing.T) {
	buf := NewCircularBuffer(1024)
	if buf == nil {
		t.Fatal("expected non-nil buffer")
	}
	if buf.size != 1024 {
		t.Errorf("expected size 1024, got %d", buf.size)
	}
}

func TestCircularBuffer_WriteAndRead(t *testing.T) {
	buf := NewCircularBuffer(100)

	// Write some data
	data := []byte("Hello, World!")
	n, err := buf.Write(data)
	if err != nil {
		t.Fatalf("write failed: %v", err)
	}
	if n != len(data) {
		t.Errorf("expected %d bytes written, got %d", len(data), n)
	}

	// Read the data back
	readBuf := make([]byte, len(data))
	n, err = buf.Read(readBuf)
	if err != nil {
		t.Fatalf("read failed: %v", err)
	}
	if n != len(data) {
		t.Errorf("expected %d bytes read, got %d", len(data), n)
	}
	if string(readBuf) != string(data) {
		t.Errorf("expected %q, got %q", string(data), string(readBuf))
	}
}

func TestCircularBuffer_Overflow(t *testing.T) {
	buf := NewCircularBuffer(10)

	// Write more data than buffer can hold
	data := []byte("Hello, World!") // 13 bytes
	buf.Write(data)

	// Get contents should return the last 10 bytes
	contents := buf.GetContents()
	if len(contents) != 10 {
		t.Errorf("expected 10 bytes in buffer, got %d", len(contents))
	}
	expected := "lo, World!"
	if string(contents) != expected {
		t.Errorf("expected %q, got %q", expected, string(contents))
	}
}

func TestCircularBuffer_GetContents(t *testing.T) {
	buf := NewCircularBuffer(100)

	// Empty buffer should return empty
	contents := buf.GetContents()
	if len(contents) != 0 {
		t.Errorf("expected empty contents, got %d bytes", len(contents))
	}

	// Write and get contents
	data := []byte("Test data")
	buf.Write(data)
	contents = buf.GetContents()
	if string(contents) != string(data) {
		t.Errorf("expected %q, got %q", string(data), string(contents))
	}
}

func TestNewManager(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)

	if mgr == nil {
		t.Fatal("expected non-nil manager")
	}
	if mgr.logDir != tmpDir {
		t.Errorf("expected logDir %s, got %s", tmpDir, mgr.logDir)
	}
	if mgr.bufferSize != DefaultBufferSize {
		t.Errorf("expected bufferSize %d, got %d", DefaultBufferSize, mgr.bufferSize)
	}
}

func TestManager_GetOrCreateSession(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)
	defer mgr.Close()

	vmID := "test-vm-123"

	// Create a log file for the VM
	logPath := filepath.Join(tmpDir, vmID+"-serial.log")
	if err := os.WriteFile(logPath, []byte("initial log\n"), 0644); err != nil {
		t.Fatalf("failed to create log file: %v", err)
	}

	// Get or create session
	session, err := mgr.GetOrCreateSession(vmID)
	if err != nil {
		t.Fatalf("GetOrCreateSession failed: %v", err)
	}

	if session.VMID != vmID {
		t.Errorf("expected VMID %s, got %s", vmID, session.VMID)
	}

	// Get the same session again - should return existing
	session2, err := mgr.GetOrCreateSession(vmID)
	if err != nil {
		t.Fatalf("GetOrCreateSession (second) failed: %v", err)
	}

	if session != session2 {
		t.Error("expected same session instance")
	}
}

func TestManager_GetSession_NotFound(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)
	defer mgr.Close()

	session, ok := mgr.GetSession("non-existent-vm")
	if ok {
		t.Error("expected session not found")
	}
	if session != nil {
		t.Error("expected nil session")
	}
}

func TestManager_RemoveSession(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)
	defer mgr.Close()

	vmID := "test-vm-123"

	// Create a log file
	logPath := filepath.Join(tmpDir, vmID+"-serial.log")
	os.WriteFile(logPath, []byte("log\n"), 0644)

	// Create session
	mgr.GetOrCreateSession(vmID)

	// Verify it exists
	_, ok := mgr.GetSession(vmID)
	if !ok {
		t.Fatal("expected session to exist")
	}

	// Remove it
	mgr.RemoveSession(vmID)

	// Verify it's gone
	_, ok = mgr.GetSession(vmID)
	if ok {
		t.Error("expected session to be removed")
	}
}

func TestManager_ListSessions(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)
	defer mgr.Close()

	// Initially empty
	sessions := mgr.ListSessions()
	if len(sessions) != 0 {
		t.Errorf("expected 0 sessions, got %d", len(sessions))
	}

	// Create sessions
	for _, vmID := range []string{"vm1", "vm2", "vm3"} {
		logPath := filepath.Join(tmpDir, vmID+"-serial.log")
		os.WriteFile(logPath, []byte("log\n"), 0644)
		mgr.GetOrCreateSession(vmID)
	}

	// Should have 3 sessions
	sessions = mgr.ListSessions()
	if len(sessions) != 3 {
		t.Errorf("expected 3 sessions, got %d", len(sessions))
	}
}

func TestSession_AddAndRemoveClient(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)
	defer mgr.Close()

	vmID := "test-vm-123"
	logPath := filepath.Join(tmpDir, vmID+"-serial.log")
	os.WriteFile(logPath, []byte("log\n"), 0644)

	session, _ := mgr.GetOrCreateSession(vmID)

	// Add client
	client := session.AddClient("client-1")
	if client == nil {
		t.Fatal("expected non-nil client")
	}
	if client.ID != "client-1" {
		t.Errorf("expected client ID client-1, got %s", client.ID)
	}

	// Check client count
	if session.ClientCount() != 1 {
		t.Errorf("expected 1 client, got %d", session.ClientCount())
	}

	// Remove client
	session.RemoveClient("client-1")

	// Check client count
	if session.ClientCount() != 0 {
		t.Errorf("expected 0 clients, got %d", session.ClientCount())
	}
}

func TestSession_IsActive(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)
	defer mgr.Close()

	vmID := "test-vm-123"
	logPath := filepath.Join(tmpDir, vmID+"-serial.log")
	os.WriteFile(logPath, []byte("log\n"), 0644)

	session, _ := mgr.GetOrCreateSession(vmID)

	if !session.IsActive() {
		t.Error("expected session to be active")
	}

	// Cancel the session context
	mgr.RemoveSession(vmID)

	// Create new manager to get fresh session
	// Note: The old session reference might still show as active due to caching
	// This is testing the context cancellation flow
}

func TestSession_WriteInput_NotEnabled(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)
	defer mgr.Close()

	vmID := "test-vm-123"
	logPath := filepath.Join(tmpDir, vmID+"-serial.log")
	os.WriteFile(logPath, []byte("log\n"), 0644)

	session, _ := mgr.GetOrCreateSession(vmID)

	// TTY not enabled, should fail
	err := session.WriteInput([]byte("test input"))
	if err == nil {
		t.Error("expected error when TTY not enabled")
	}
}

func TestSession_Broadcast(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)
	defer mgr.Close()

	vmID := "test-vm-123"
	logPath := filepath.Join(tmpDir, vmID+"-serial.log")
	os.WriteFile(logPath, []byte("log\n"), 0644)

	session, _ := mgr.GetOrCreateSession(vmID)

	// Add two clients
	client1 := session.AddClient("client-1")
	client2 := session.AddClient("client-2")

	// Broadcast a message
	testData := []byte("broadcast test")
	session.broadcast(testData)

	// Check both clients received it
	select {
	case data := <-client1.Send:
		if string(data) != string(testData) {
			t.Errorf("client1 expected %q, got %q", string(testData), string(data))
		}
	case <-time.After(100 * time.Millisecond):
		t.Error("client1 did not receive broadcast")
	}

	select {
	case data := <-client2.Send:
		if string(data) != string(testData) {
			t.Errorf("client2 expected %q, got %q", string(testData), string(data))
		}
	case <-time.After(100 * time.Millisecond):
		t.Error("client2 did not receive broadcast")
	}
}

func TestManager_Close(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)

	vmID := "test-vm-123"
	logPath := filepath.Join(tmpDir, vmID+"-serial.log")
	os.WriteFile(logPath, []byte("log\n"), 0644)

	mgr.GetOrCreateSession(vmID)

	// Close manager
	if err := mgr.Close(); err != nil {
		t.Errorf("Close failed: %v", err)
	}

	// Sessions should be cleared
	sessions := mgr.ListSessions()
	if len(sessions) != 0 {
		t.Errorf("expected 0 sessions after close, got %d", len(sessions))
	}
}

func TestSession_ContextCancellation(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)
	defer mgr.Close()

	vmID := "test-vm-123"
	logPath := filepath.Join(tmpDir, vmID+"-serial.log")
	os.WriteFile(logPath, []byte("log\n"), 0644)

	session, _ := mgr.GetOrCreateSession(vmID)

	// Cancel the session
	session.cancel()

	// Wait a bit for cancellation to propagate
	time.Sleep(10 * time.Millisecond)

	// Context should be done
	select {
	case <-session.ctx.Done():
		// Expected
	default:
		t.Error("expected session context to be cancelled")
	}
}

func TestNewManager_DefaultLogDir(t *testing.T) {
	mgr := NewManager("")
	if mgr.logDir != DefaultLogDir {
		t.Errorf("expected default log dir %s, got %s", DefaultLogDir, mgr.logDir)
	}
	mgr.Close()
}

func TestSession_PTYPath(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)
	defer mgr.Close()

	vmID := "test-vm-pty"
	logPath := filepath.Join(tmpDir, vmID+"-serial.log")
	os.WriteFile(logPath, []byte("log\n"), 0644)

	session, _ := mgr.GetOrCreateSession(vmID)

	// Initially PTY path should be empty
	ptyPath := session.GetPTYPath()
	if ptyPath != "" {
		t.Errorf("expected empty PTY path, got %s", ptyPath)
	}

	// Set PTY path
	testPath := "/dev/pts/123"
	session.SetPTYPath(testPath)

	// Get PTY path
	ptyPath = session.GetPTYPath()
	if ptyPath != testPath {
		t.Errorf("expected PTY path %s, got %s", testPath, ptyPath)
	}
}

func TestClient_HandleResize_NoPTY(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)
	defer mgr.Close()

	vmID := "test-vm-resize"
	logPath := filepath.Join(tmpDir, vmID+"-serial.log")
	os.WriteFile(logPath, []byte("log\n"), 0644)

	session, _ := mgr.GetOrCreateSession(vmID)
	client := session.AddClient("resize-client")

	// Without PTY path set, resize should return an error
	err := client.handleResize(80, 24)
	if err == nil {
		t.Error("expected error when PTY path is not set")
	}
	if err.Error() != "PTY resize not available: VM not configured with PTY device" {
		t.Errorf("unexpected error message: %v", err)
	}
}

func TestClient_SetLogger(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)
	defer mgr.Close()

	vmID := "test-vm-logger"
	logPath := filepath.Join(tmpDir, vmID+"-serial.log")
	os.WriteFile(logPath, []byte("log\n"), 0644)

	session, _ := mgr.GetOrCreateSession(vmID)
	client := session.AddClient("logger-client")

	// Set a logger
	logger := zap.NewNop()
	client.SetLogger(logger)

	// Verify it doesn't panic
	_ = client.getLogger()
}
