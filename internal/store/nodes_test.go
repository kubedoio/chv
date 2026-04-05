package store

import (
	"context"
	"strings"
	"testing"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgconn"
)

// mockQuerier is a mock implementation of the querier interface for testing
type mockQuerier struct {
	queryFunc    func(ctx context.Context, sql string, args ...interface{}) (pgx.Rows, error)
	queryRowFunc func(ctx context.Context, sql string, args ...interface{}) pgx.Row
	execFunc     func(ctx context.Context, sql string, args ...interface{}) (pgconn.CommandTag, error)
}

func (m *mockQuerier) Query(ctx context.Context, sql string, args ...interface{}) (pgx.Rows, error) {
	if m.queryFunc != nil {
		return m.queryFunc(ctx, sql, args...)
	}
	return nil, nil
}

func (m *mockQuerier) QueryRow(ctx context.Context, sql string, args ...interface{}) pgx.Row {
	if m.queryRowFunc != nil {
		return m.queryRowFunc(ctx, sql, args...)
	}
	return nil
}

func (m *mockQuerier) Exec(ctx context.Context, sql string, args ...interface{}) (pgconn.CommandTag, error) {
	if m.execFunc != nil {
		return m.execFunc(ctx, sql, args...)
	}
	return pgconn.CommandTag{}, nil
}

// mockRow implements pgx.Row for testing
type mockRow struct {
	scanFunc func(dest ...interface{}) error
}

func (m *mockRow) Scan(dest ...interface{}) error {
	if m.scanFunc != nil {
		return m.scanFunc(dest...)
	}
	return nil
}

// mockRows implements pgx.Rows for testing
type mockRows struct {
	nextFunc  func() bool
	scanFunc  func(dest ...interface{}) error
	closeFunc func()
	errFunc   func() error
	closed    bool
	callCount int
}

func (m *mockRows) Next() bool {
	if m.nextFunc != nil {
		return m.nextFunc()
	}
	return false
}

func (m *mockRows) Scan(dest ...interface{}) error {
	if m.scanFunc != nil {
		return m.scanFunc(dest...)
	}
	return nil
}

func (m *mockRows) Close() {
	if m.closeFunc != nil {
		m.closeFunc()
	}
	m.closed = true
}

func (m *mockRows) Err() error {
	if m.errFunc != nil {
		return m.errFunc()
	}
	return nil
}

// RawValues is needed to satisfy pgx.Rows interface
func (m *mockRows) RawValues() [][]byte { return nil }

// Conn is needed to satisfy pgx.Rows interface
func (m *mockRows) Conn() *pgx.Conn { return nil }

// CommandTag is needed to satisfy pgx.Rows interface
func (m *mockRows) CommandTag() pgconn.CommandTag { return pgconn.CommandTag{} }

// FieldDescriptions is needed to satisfy pgx.Rows interface
func (m *mockRows) FieldDescriptions() []pgconn.FieldDescription { return nil }

// Values is needed to satisfy pgx.Rows interface
func (m *mockRows) Values() ([]interface{}, error) { return nil, nil }

// TestListNodes_Success verifies that listNodes properly handles inet type scanning via ::text cast
func TestListNodes_Success(t *testing.T) {
	ctx := context.Background()
	now := time.Now()
	nodeID := uuid.New()

	callCount := 0
	mockRows := &mockRows{
		nextFunc: func() bool {
			callCount++
			return callCount == 1
		},
		scanFunc: func(dest ...interface{}) error {
			// Simulate scanning a row with management_ip as text (after ::text cast)
			// This test verifies the fix for: "can't scan into dest[2]: cannot scan inet (OID 869) in binary format into *string"
			*dest[0].(*uuid.UUID) = nodeID
			*dest[1].(*string) = "test-node-1"
			*dest[2].(*string) = "192.168.1.100" // This is the key: inet cast to text
			*dest[3].(*models.NodeState) = models.NodeStateOnline
			*dest[4].(*bool) = false
			*dest[5].(*int32) = 8
			*dest[6].(*int64) = 16384
			*dest[7].(*int32) = 6
			*dest[8].(*int64) = 12288
			// Labels and Capabilities are json.RawMessage - handled by the driver
			*dest[11].(*string) = "1.0.0"
			*dest[12].(*string) = "chv 0.1.0"
			// dest[13] is **time.Time (LastHeartbeatAt is *time.Time)
			*(dest[13].(**time.Time)) = &now
			*dest[14].(*time.Time) = now
			*dest[15].(*time.Time) = now
			return nil
		},
	}

	mock := &mockQuerier{
		queryFunc: func(ctx context.Context, sql string, args ...interface{}) (pgx.Rows, error) {
			// Verify the SQL contains the ::text cast - this is the fix being tested
			if !strings.Contains(sql, "management_ip::text") {
				t.Errorf("SQL query missing management_ip::text cast: %s", sql)
			}
			return mockRows, nil
		},
	}

	nodes, err := listNodes(ctx, mock)
	if err != nil {
		t.Errorf("unexpected error: %v", err)
		return
	}

	if len(nodes) != 1 {
		t.Errorf("expected 1 node, got %d", len(nodes))
		return
	}

	if nodes[0].ManagementIP != "192.168.1.100" {
		t.Errorf("expected ManagementIP 192.168.1.100, got %s", nodes[0].ManagementIP)
	}

	// Verify rows were closed
	if !mockRows.closed {
		t.Error("expected rows to be closed")
	}
}

// TestListNodes_Empty verifies that listNodes returns empty slice when no nodes exist
func TestListNodes_Empty(t *testing.T) {
	ctx := context.Background()

	mockRows := &mockRows{
		nextFunc: func() bool { return false },
	}

	mock := &mockQuerier{
		queryFunc: func(ctx context.Context, sql string, args ...interface{}) (pgx.Rows, error) {
			if !strings.Contains(sql, "management_ip::text") {
				t.Errorf("SQL query missing management_ip::text cast: %s", sql)
			}
			return mockRows, nil
		},
	}

	nodes, err := listNodes(ctx, mock)
	if err != nil {
		t.Errorf("unexpected error: %v", err)
		return
	}

	if len(nodes) != 0 {
		t.Errorf("expected 0 nodes, got %d", len(nodes))
	}
}

// TestGetNode_InetScanning verifies that getNode properly handles inet type scanning
func TestGetNode_InetScanning(t *testing.T) {
	ctx := context.Background()
	nodeID := uuid.New()
	now := time.Now()

	mock := &mockQuerier{
		queryRowFunc: func(ctx context.Context, sql string, args ...interface{}) pgx.Row {
			// Verify the SQL contains the ::text cast
			if !strings.Contains(sql, "management_ip::text") {
				t.Errorf("SQL query missing management_ip::text cast: %s", sql)
			}
			return &mockRow{
				scanFunc: func(dest ...interface{}) error {
					*dest[0].(*uuid.UUID) = nodeID
					*dest[1].(*string) = "test-node"
					*dest[2].(*string) = "10.0.0.1" // inet as text
					*dest[3].(*models.NodeState) = models.NodeStateOnline
					*dest[4].(*bool) = false
					*dest[5].(*int32) = 4
					*dest[6].(*int64) = 8192
					*dest[7].(*int32) = 3
					*dest[8].(*int64) = 6144
					*dest[11].(*string) = "1.0.0"
					*dest[12].(*string) = "chv 0.1.0"
					*(dest[13].(**time.Time)) = &now
					*dest[14].(*time.Time) = now
					*dest[15].(*time.Time) = now
					return nil
				},
			}
		},
	}

	node, err := getNode(ctx, mock, nodeID)
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if node == nil {
		t.Fatal("expected node, got nil")
	}
	if node.ManagementIP != "10.0.0.1" {
		t.Errorf("expected ManagementIP 10.0.0.1, got %s", node.ManagementIP)
	}
}

// TestGetNodeByHostname_InetScanning verifies that getNodeByHostname properly handles inet type scanning
func TestGetNodeByHostname_InetScanning(t *testing.T) {
	ctx := context.Background()
	nodeID := uuid.New()
	now := time.Now()

	mock := &mockQuerier{
		queryRowFunc: func(ctx context.Context, sql string, args ...interface{}) pgx.Row {
			// Verify the SQL contains the ::text cast
			if !strings.Contains(sql, "management_ip::text") {
				t.Errorf("SQL query missing management_ip::text cast: %s", sql)
			}
			return &mockRow{
				scanFunc: func(dest ...interface{}) error {
					*dest[0].(*uuid.UUID) = nodeID
					*dest[1].(*string) = "test-hostname"
					*dest[2].(*string) = "192.168.0.1/24" // inet with CIDR as text
					*dest[3].(*models.NodeState) = models.NodeStateOnline
					*dest[4].(*bool) = false
					*dest[5].(*int32) = 4
					*dest[6].(*int64) = 8192
					*dest[7].(*int32) = 3
					*dest[8].(*int64) = 6144
					*dest[11].(*string) = "1.0.0"
					*dest[12].(*string) = "chv 0.1.0"
					*(dest[13].(**time.Time)) = &now
					*dest[14].(*time.Time) = now
					*dest[15].(*time.Time) = now
					return nil
				},
			}
		},
	}

	node, err := getNodeByHostname(ctx, mock, "test-hostname")
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if node == nil {
		t.Fatal("expected node, got nil")
	}
	if node.ManagementIP != "192.168.0.1/24" {
		t.Errorf("expected ManagementIP 192.168.0.1/24, got %s", node.ManagementIP)
	}
}

// TestInetToTextCast verifies that the SQL queries contain the required ::text cast for management_ip
// This is a regression test for the health check bug:
// "can't scan into dest[2]: cannot scan inet (OID 869) in binary format into *string"
func TestInetToTextCast(t *testing.T) {
	// This test verifies the fix is in place by checking the SQL strings
	// If the ::text cast is missing, scanning inet into a string field will fail

	ctx := context.Background()
	nodeID := uuid.New()

	// Test getNode SQL contains ::text cast
	var getNodeSQL string
	mockGetNode := &mockQuerier{
		queryRowFunc: func(ctx context.Context, sql string, args ...interface{}) pgx.Row {
			getNodeSQL = sql
			return &mockRow{scanFunc: func(dest ...interface{}) error { return pgx.ErrNoRows }}
		},
	}
	getNode(ctx, mockGetNode, nodeID)
	if !strings.Contains(getNodeSQL, "management_ip::text") {
		t.Errorf("getNode SQL missing management_ip::text cast: %s", getNodeSQL)
	}

	// Test getNodeByHostname SQL contains ::text cast
	var getNodeByHostnameSQL string
	mockGetNodeByHostname := &mockQuerier{
		queryRowFunc: func(ctx context.Context, sql string, args ...interface{}) pgx.Row {
			getNodeByHostnameSQL = sql
			return &mockRow{scanFunc: func(dest ...interface{}) error { return pgx.ErrNoRows }}
		},
	}
	getNodeByHostname(ctx, mockGetNodeByHostname, "test")
	if !strings.Contains(getNodeByHostnameSQL, "management_ip::text") {
		t.Errorf("getNodeByHostname SQL missing management_ip::text cast: %s", getNodeByHostnameSQL)
	}

	// Test listNodes SQL contains ::text cast
	var listNodesSQL string
	mockListNodes := &mockQuerier{
		queryFunc: func(ctx context.Context, sql string, args ...interface{}) (pgx.Rows, error) {
			listNodesSQL = sql
			return &mockRows{nextFunc: func() bool { return false }}, nil
		},
	}
	listNodes(ctx, mockListNodes)
	if !strings.Contains(listNodesSQL, "management_ip::text") {
		t.Errorf("listNodes SQL missing management_ip::text cast: %s", listNodesSQL)
	}
}
