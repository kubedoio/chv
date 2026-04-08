#!/bin/bash

# CHV Startup Script

set -e

CHV_DIR="/srv/data02/projects/chv"
DATA_ROOT="/var/lib/chv"
LOG_DIR="$DATA_ROOT/logs"

# Create necessary directories
mkdir -p $DATA_ROOT/{images,vms,seed,storage/localdisk,logs}

# Stop any existing services
echo "Stopping any existing CHV services..."
pkill -f chv-controller 2>/dev/null || true
pkill -f chv-agent 2>/dev/null || true
sleep 2

# Start Agent
echo "Starting CHV Agent..."
CHV_AGENT_ADDR=:9090 \
CHV_DATA_ROOT=$DATA_ROOT \
CHV_LOG_LEVEL=info \
CHV_BRIDGE_NAME=chvbr0 \
$CHV_DIR/chv-agent > $LOG_DIR/agent.log 2>&1 &

AGENT_PID=$!
echo "Agent started with PID: $AGENT_PID"

# Wait for agent to be ready
sleep 2
if curl -s http://localhost:9090/health > /dev/null; then
    echo "Agent is healthy"
else
    echo "Agent failed to start. Check logs at $LOG_DIR/agent.log"
    exit 1
fi

# Start Controller
echo "Starting CHV Controller..."
CHV_HTTP_ADDR=:8888 \
CHV_DATA_ROOT=$DATA_ROOT \
CHV_AGENT_URL=http://localhost:9090 \
CHV_LOG_LEVEL=info \
CHV_LOG_DIR=$LOG_DIR \
$CHV_DIR/chv-controller > $LOG_DIR/controller.log 2>&1 &

CONTROLLER_PID=$!
echo "Controller started with PID: $CONTROLLER_PID"

# Wait for controller to be ready
sleep 2
if curl -s http://localhost:8888/health > /dev/null; then
    echo "Controller is healthy"
else
    echo "Controller failed to start. Check logs at $LOG_DIR/controller.log"
    exit 1
fi

echo ""
echo "=========================================="
echo "CHV is now running!"
echo "=========================================="
echo ""
echo "WebUI:        http://localhost:8888"
echo "Agent API:    http://localhost:9090"
echo "Controller:   http://localhost:8888"
echo ""
echo "Logs:"
echo "  Agent:      $LOG_DIR/agent.log"
echo "  Controller: $LOG_DIR/controller.log"
echo ""
echo "Data:         $DATA_ROOT"
echo ""
echo "To stop:      pkill -f chv-controller; pkill -f chv-agent"
echo "=========================================="
