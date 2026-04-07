package network

import (
	"context"
	"errors"
	"testing"
)

type fakeRunner struct {
	outputs map[string][]byte
	errs    map[string]error
	calls   []string
}

func (f *fakeRunner) Run(_ context.Context, name string, args ...string) ([]byte, error) {
	key := name
	for _, arg := range args {
		key += " " + arg
	}
	f.calls = append(f.calls, key)
	if err, ok := f.errs[key]; ok {
		return nil, err
	}
	return f.outputs[key], nil
}

func (f *fakeRunner) LookPath(file string) (string, error) {
	key := "lookpath " + file
	if err, ok := f.errs[key]; ok {
		return "", err
	}
	if out, ok := f.outputs[key]; ok {
		return string(out), nil
	}
	return "", errors.New("not found")
}

func TestBridgeManagerInspectDetectsDrift(t *testing.T) {
	runner := &fakeRunner{
		outputs: map[string][]byte{
			"ip link show chvbr0":        []byte("2: chvbr0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 state UP"),
			"ip -4 addr show dev chvbr0": []byte("    inet 10.0.1.1/24 brd 10.0.1.255 scope global chvbr0\n"),
			"lookpath cloud-hypervisor":  []byte("/usr/bin/cloud-hypervisor"),
			"lookpath xorrisofs":         []byte("/usr/bin/xorrisofs"),
		},
	}

	manager := NewBridgeManager(runner)
	status, err := manager.Inspect(context.Background(), "chvbr0", "10.0.0.1/24")
	if err != nil {
		t.Fatalf("Inspect() error = %v", err)
	}

	if !status.Exists {
		t.Fatalf("expected bridge to exist")
	}
	if status.ActualIP != "10.0.1.1/24" {
		t.Fatalf("expected actual IP 10.0.1.1/24, got %q", status.ActualIP)
	}
	if !status.Drift {
		t.Fatalf("expected drift to be detected")
	}
}

func TestBridgeManagerEnsureCreatesMissingBridge(t *testing.T) {
	runner := &fakeRunner{
		outputs: map[string][]byte{
			"ip link add chvbr0 type bridge":     []byte(""),
			"ip addr add 10.0.0.1/24 dev chvbr0": []byte(""),
			"ip link set chvbr0 up":              []byte(""),
		},
		errs: map[string]error{
			"ip link show chvbr0": errors.New("missing bridge"),
		},
	}

	manager := NewBridgeManager(runner)
	actions, err := manager.Ensure(context.Background(), "chvbr0", "10.0.0.1/24")
	if err != nil {
		t.Fatalf("Ensure() error = %v", err)
	}

	if len(actions) != 3 {
		t.Fatalf("expected 3 actions, got %d (%v)", len(actions), actions)
	}
}
