// Package console provides VM serial console streaming for the agent.
package console

import (
	"bufio"
	"context"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sync"
	"time"

	"go.uber.org/zap"
	"golang.org/x/sys/unix"
)

const (
	// DefaultBufferSize is the size of the circular buffer for recent console output
	DefaultBufferSize = 100 * 1024 // 100KB buffer

	// DefaultLogDir is the default directory for CH serial console logs
	DefaultLogDir = "/var/log/chv"

	// MaxLineLength is the maximum length of a single console line
	MaxLineLength = 4096
)

// Manager manages VM serial console streaming and buffering.
type Manager struct {
	logDir     string
	bufferSize int
	sessions   map[string]*Session // key: vmID
	mu         sync.RWMutex
	ctx        context.Context
	cancel     context.CancelFunc
}

// Session represents an active console session for a VM.
type Session struct {
	VMID         string
	LogPath      string
	Buffer       *CircularBuffer
	Clients      map[string]*Client // key: clientID
	mu           sync.RWMutex
	ctx          context.Context
	cancel       context.CancelFunc
	ttyEnabled   bool
	inputPath    string // Path to write input (if TTY mode)
	ptyPath      string // Path to PTY device (if TTY mode)
	lastActivity time.Time
}

// GetPTYPath returns the PTY device path for this session.
func (s *Session) GetPTYPath() string {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.ptyPath
}

// SetPTYPath sets the PTY device path for this session.
func (s *Session) SetPTYPath(path string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.ptyPath = path
}

// Client represents a connected console client.
type Client struct {
	ID        string
	Send      chan []byte
	Recv      chan []byte
	Session   *Session
	Connected bool
	mu        sync.Mutex
	logger    *zap.Logger
}

// SetLogger sets the logger for the client.
func (c *Client) SetLogger(logger *zap.Logger) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.logger = logger
}

// getLogger returns the logger for the client, or a no-op logger if not set.
func (c *Client) getLogger() *zap.Logger {
	c.mu.Lock()
	defer c.mu.Unlock()
	if c.logger == nil {
		return zap.NewNop()
	}
	return c.logger
}

// handleResize resizes the PTY associated with this client's session.
// It uses the TIOCSWINSZ ioctl to resize the PTY.
func (c *Client) handleResize(cols, rows int) error {
	// Get logger first (needs lock)
	logger := c.getLogger()

	// Check if we have a session
	if c.Session == nil {
		return fmt.Errorf("no session available for resize")
	}

	// Check if PTY is available
	ptyPath := c.Session.GetPTYPath()
	if ptyPath == "" {
		// PTY resize requires the VM to be configured with a PTY device.
		// The current cloud-hypervisor configuration uses --serial tty which
		// outputs to stdout rather than a PTY.
		return fmt.Errorf("PTY resize not available: VM not configured with PTY device")
	}

	// Open the PTY device
	ptyFile, err := os.OpenFile(ptyPath, os.O_RDWR, 0)
	if err != nil {
		return fmt.Errorf("failed to open PTY %s: %w", ptyPath, err)
	}
	defer ptyFile.Close()

	// Use TIOCSWINSZ ioctl to resize the PTY
	ws := &unix.Winsize{
		Col: uint16(cols),
		Row: uint16(rows),
	}

	err = unix.IoctlSetWinsize(int(ptyFile.Fd()), unix.TIOCSWINSZ, ws)
	if err != nil {
		return fmt.Errorf("failed to resize PTY: %w", err)
	}

	logger.Debug("Console resized",
		zap.Int("cols", cols),
		zap.Int("rows", rows),
		zap.String("pty", ptyPath))

	return nil
}

// CircularBuffer is a thread-safe circular buffer for storing recent console output.
type CircularBuffer struct {
	data  []byte
	size  int
	start int
	end   int
	full  bool
	mu    sync.RWMutex
}

// NewCircularBuffer creates a new circular buffer with the given size.
func NewCircularBuffer(size int) *CircularBuffer {
	return &CircularBuffer{
		data: make([]byte, size),
		size: size,
	}
}

// Write writes data to the circular buffer.
func (b *CircularBuffer) Write(p []byte) (n int, err error) {
	b.mu.Lock()
	defer b.mu.Unlock()

	for _, c := range p {
		b.data[b.end] = c
		b.end = (b.end + 1) % b.size
		if b.full {
			b.start = (b.start + 1) % b.size
		} else if b.end == b.start {
			b.full = true
		}
	}
	return len(p), nil
}

// Read reads data from the circular buffer into p.
// Returns the number of bytes read.
func (b *CircularBuffer) Read(p []byte) (n int, err error) {
	b.mu.RLock()
	defer b.mu.RUnlock()

	if b.start == b.end && !b.full {
		return 0, io.EOF
	}

	written := 0
	for written < len(p) && (b.start != b.end || b.full) {
		p[written] = b.data[b.start]
		b.start = (b.start + 1) % b.size
		written++
		b.full = false
	}

	return written, nil
}

// GetContents returns all contents of the buffer as a byte slice.
// This is a non-destructive read that doesn't modify the buffer state.
func (b *CircularBuffer) GetContents() []byte {
	b.mu.RLock()
	defer b.mu.RUnlock()

	if b.start == b.end && !b.full {
		return []byte{}
	}

	if b.full && b.end == b.start {
		// Buffer is completely full and wrapped
		result := make([]byte, b.size)
		firstPart := b.size - b.start
		copy(result[:firstPart], b.data[b.start:])
		copy(result[firstPart:], b.data[:b.end])
		return result
	}

	if b.start < b.end {
		// Simple case: no wrap
		result := make([]byte, b.end-b.start)
		copy(result, b.data[b.start:b.end])
		return result
	}

	// Wrapped around but not full
	firstPart := b.size - b.start
	result := make([]byte, firstPart+b.end)
	copy(result[:firstPart], b.data[b.start:])
	copy(result[firstPart:], b.data[:b.end])
	return result
}

// NewManager creates a new console manager.
func NewManager(logDir string) *Manager {
	if logDir == "" {
		logDir = DefaultLogDir
	}

	ctx, cancel := context.WithCancel(context.Background())

	return &Manager{
		logDir:     logDir,
		bufferSize: DefaultBufferSize,
		sessions:   make(map[string]*Session),
		ctx:        ctx,
		cancel:     cancel,
	}
}

// GetOrCreateSession gets an existing session or creates a new one for the VM.
func (m *Manager) GetOrCreateSession(vmID string) (*Session, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	if session, ok := m.sessions[vmID]; ok {
		session.lastActivity = time.Now()
		return session, nil
	}

	// Create new session
	logPath := filepath.Join(m.logDir, fmt.Sprintf("%s-serial.log", vmID))

	// Check if log file exists or can be created
	if _, err := os.Stat(logPath); err != nil && !os.IsNotExist(err) {
		return nil, fmt.Errorf("cannot access console log for VM %s: %w", vmID, err)
	}

	ctx, cancel := context.WithCancel(m.ctx)
	session := &Session{
		VMID:         vmID,
		LogPath:      logPath,
		Buffer:       NewCircularBuffer(m.bufferSize),
		Clients:      make(map[string]*Client),
		ctx:          ctx,
		cancel:       cancel,
		lastActivity: time.Now(),
	}

	m.sessions[vmID] = session

	// Start tailing the log file
	go session.tailLog()

	return session, nil
}

// GetSession gets an existing session without creating a new one.
func (m *Manager) GetSession(vmID string) (*Session, bool) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	session, ok := m.sessions[vmID]
	return session, ok
}

// RemoveSession removes a session and stops tailing.
func (m *Manager) RemoveSession(vmID string) {
	m.mu.Lock()
	defer m.mu.Unlock()

	if session, ok := m.sessions[vmID]; ok {
		session.cancel()
		delete(m.sessions, vmID)
	}
}

// Close closes the manager and all sessions.
func (m *Manager) Close() error {
	m.cancel()
	m.mu.Lock()
	defer m.mu.Unlock()

	for _, session := range m.sessions {
		session.cancel()
	}
	m.sessions = make(map[string]*Session)
	return nil
}

// tailLog tails the log file and broadcasts to clients.
func (s *Session) tailLog() {
	// Wait for file to exist if it doesn't yet
	ticker := time.NewTicker(100 * time.Millisecond)
	defer ticker.Stop()

	for {
		select {
		case <-s.ctx.Done():
			return
		case <-ticker.C:
			if _, err := os.Stat(s.LogPath); err == nil {
				s.startTailing()
				return
			}
		}
	}
}

// startTailing starts tailing the log file.
func (s *Session) startTailing() {
	file, err := os.Open(s.LogPath)
	if err != nil {
		// Log error but keep trying
		time.Sleep(1 * time.Second)
		go s.tailLog()
		return
	}
	defer file.Close()

	// Seek to end to tail new content only
	// For new sessions, we might want to seek to beginning to get history
	// For reconnecting clients, we'd want to seek to a specific offset
	// For now, we'll read from the beginning and buffer the content
	file.Seek(0, io.SeekStart)

	reader := bufio.NewReaderSize(file, MaxLineLength)

	for {
		select {
		case <-s.ctx.Done():
			return
		default:
		}

		line, err := reader.ReadBytes('\n')
		if err != nil {
			if err == io.EOF {
				// Wait for new data
				time.Sleep(100 * time.Millisecond)
				continue
			}
			// File may have been rotated or removed
			time.Sleep(1 * time.Second)
			go s.tailLog()
			return
		}

		// Write to buffer
		s.Buffer.Write(line)

		// Broadcast to clients
		s.broadcast(line)
		s.lastActivity = time.Now()
	}
}

// broadcast sends data to all connected clients.
func (s *Session) broadcast(data []byte) {
	s.mu.RLock()
	clients := make([]*Client, 0, len(s.Clients))
	for _, client := range s.Clients {
		clients = append(clients, client)
	}
	s.mu.RUnlock()

	for _, client := range clients {
		select {
		case client.Send <- data:
		default:
			// Client channel full, drop message (client is slow)
		}
	}
}

// AddClient adds a client to the session.
func (s *Session) AddClient(clientID string) *Client {
	s.mu.Lock()
	defer s.mu.Unlock()

	client := &Client{
		ID:        clientID,
		Send:      make(chan []byte, 100),
		Recv:      make(chan []byte, 10),
		Session:   s,
		Connected: true,
	}

	s.Clients[clientID] = client
	s.lastActivity = time.Now()

	return client
}

// RemoveClient removes a client from the session.
func (s *Session) RemoveClient(clientID string) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if client, ok := s.Clients[clientID]; ok {
		client.mu.Lock()
		client.Connected = false
		close(client.Send)
		close(client.Recv)
		client.mu.Unlock()
		delete(s.Clients, clientID)
	}

	// If no more clients, consider closing session after timeout
	if len(s.Clients) == 0 {
		go s.scheduleCleanup()
	}
}

// scheduleCleanup schedules session cleanup after a period of inactivity.
func (s *Session) scheduleCleanup() {
	time.Sleep(5 * time.Minute)

	s.mu.RLock()
	clientCount := len(s.Clients)
	s.mu.RUnlock()

	if clientCount == 0 {
		s.cancel()
	}
}

// GetHistory returns the buffered console history.
func (s *Session) GetHistory() []byte {
	return s.Buffer.GetContents()
}

// WriteInput writes input to the console (if TTY mode enabled).
func (s *Session) WriteInput(data []byte) error {
	if !s.ttyEnabled || s.inputPath == "" {
		return fmt.Errorf("input not supported: TTY mode not enabled for VM %s", s.VMID)
	}

	// Write to input file/socket
	file, err := os.OpenFile(s.inputPath, os.O_WRONLY, 0)
	if err != nil {
		return fmt.Errorf("failed to open input path: %w", err)
	}
	defer file.Close()

	_, err = file.Write(data)
	return err
}

// IsActive returns true if the session is active.
func (s *Session) IsActive() bool {
	select {
	case <-s.ctx.Done():
		return false
	default:
		return true
	}
}

// ClientCount returns the number of connected clients.
func (s *Session) ClientCount() int {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return len(s.Clients)
}

// ListSessions returns a list of all active session VM IDs.
func (m *Manager) ListSessions() []string {
	m.mu.RLock()
	defer m.mu.RUnlock()

	vmIDs := make([]string, 0, len(m.sessions))
	for vmID := range m.sessions {
		vmIDs = append(vmIDs, vmID)
	}
	return vmIDs
}
