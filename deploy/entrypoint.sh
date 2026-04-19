#!/bin/sh
set -e

echo "=== CHV Starting ==="

# Start nginx (serves UI + proxies API)
nginx

# Start storage daemon
chv-stord /etc/chv/stord.toml &
STORD_PID=$!

# Start network daemon
chv-nwd /etc/chv/nwd.toml &
NWD_PID=$!

# Start control plane (includes BFF)
chv-controlplane /etc/chv/controlplane.toml &
CP_PID=$!

# Wait for controlplane to be ready before starting agent
sleep 2

# Start agent
chv-agent /etc/chv/agent.toml &
AGENT_PID=$!

echo "=== CHV Ready ==="
echo "  UI:     http://localhost:80"
echo "  API:    http://localhost:8080"
echo "  gRPC:   localhost:8443"

# Wait for any process to exit
wait -n $CP_PID $AGENT_PID $STORD_PID $NWD_PID 2>/dev/null || true
echo "=== CHV process exited, shutting down ==="
kill $CP_PID $AGENT_PID $STORD_PID $NWD_PID 2>/dev/null || true
wait
