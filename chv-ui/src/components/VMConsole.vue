<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'
import Dialog from 'primevue/dialog'
import Button from 'primevue/button'
import { useToast } from 'primevue/usetoast'

const props = defineProps<{
  vmId: string
  visible: boolean
}>()

const emit = defineEmits<{
  (e: 'close'): void
}>()

const toast = useToast()

// Template refs
const terminal = ref<HTMLElement>()

// Terminal and WebSocket state
let xterm: Terminal | null = null
let fitAddon: FitAddon | null = null
let ws: WebSocket | null = null

// Connection status: 'disconnected' | 'connecting' | 'connected' | 'error'
const connectionStatus = ref<'disconnected' | 'connecting' | 'connected' | 'error'>('disconnected')

// WebSocket URL construction
const getWebSocketUrl = (): string => {
  const API_BASE_URL = import.meta.env.VITE_API_URL || window.location.origin
  const token = localStorage.getItem('chv_token') || ''
  
  // Convert http/https to ws/wss
  let wsUrl = API_BASE_URL.replace(/^http/, 'ws')
  if (!wsUrl) {
    wsUrl = `ws://${window.location.host}`
  }
  
  return `${wsUrl}/api/v1/vms/${props.vmId}/console?token=${encodeURIComponent(token)}`
}

// Initialize xterm.js terminal
const initTerminal = () => {
  if (!terminal.value || xterm) return

  xterm = new Terminal({
    theme: {
      background: '#000000',
      foreground: '#00ff00',
      cursor: '#00ff00',
      selectionBackground: '#00ff00',
      selectionForeground: '#000000'
    },
    fontFamily: 'monospace',
    fontSize: 14,
    cursorBlink: true,
    cursorStyle: 'block'
  })

  fitAddon = new FitAddon()
  xterm.loadAddon(fitAddon)

  xterm.open(terminal.value)
  fitAddon.fit()

  // Handle user input - encode and send via WebSocket
  xterm.onData((data: string) => {
    if (ws?.readyState === WebSocket.OPEN) {
      const encoded = btoa(data)
      ws.send(JSON.stringify({
        type: 'input',
        data: encoded
      }))
    }
  })

  // Handle resize events
  window.addEventListener('resize', handleResize)
}

// Handle terminal resize
const handleResize = () => {
  if (fitAddon) {
    try {
      fitAddon.fit()
    } catch (e) {
      // Ignore fit errors
    }
  }
}

// Connect to WebSocket
const connect = () => {
  if (ws) {
    ws.close()
    ws = null
  }

  connectionStatus.value = 'connecting'

  try {
    const url = getWebSocketUrl()
    ws = new WebSocket(url)

    ws.onopen = () => {
      connectionStatus.value = 'connected'
      toast.add({
        severity: 'success',
        summary: 'Connected',
        detail: 'Console connection established',
        life: 3000
      })
    }

    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data)
        
        switch (msg.type) {
          case 'output':
            // Decode base64 output and write to terminal
            if (msg.data && xterm) {
              const decoded = atob(msg.data)
              xterm.write(decoded)
            }
            break
            
          case 'error':
            // Decode and display error
            if (msg.data) {
              const decoded = atob(msg.data)
              toast.add({
                severity: 'error',
                summary: 'Console Error',
                detail: decoded,
                life: 5000
              })
              if (xterm) {
                xterm.write(`\r\n[ERROR: ${decoded}]\r\n`)
              }
            }
            break
            
          case 'status':
            // Handle status messages
            if (msg.data) {
              const decoded = atob(msg.data)
              if (xterm && decoded !== 'pong') {
                xterm.write(`\r\n[Status: ${decoded}]\r\n`)
              }
            }
            break
            
          case 'input':
            // Echo of input, typically not displayed
            break
            
          default:
            console.warn('Unknown message type:', msg.type)
        }
      } catch (err) {
        console.error('Failed to parse WebSocket message:', err)
      }
    }

    ws.onerror = (error) => {
      console.error('WebSocket error:', error)
      connectionStatus.value = 'error'
      toast.add({
        severity: 'error',
        summary: 'Connection Error',
        detail: 'Failed to connect to VM console',
        life: 5000
      })
    }

    ws.onclose = (event) => {
      if (connectionStatus.value !== 'error') {
        connectionStatus.value = 'disconnected'
      }
      if (xterm) {
        xterm.write('\r\n[Connection closed]\r\n')
      }
    }
  } catch (err) {
    console.error('Failed to create WebSocket:', err)
    connectionStatus.value = 'error'
    toast.add({
      severity: 'error',
      summary: 'Connection Error',
      detail: 'Failed to initialize console connection',
      life: 5000
    })
  }
}

// Disconnect from WebSocket
const disconnect = () => {
  if (ws) {
    ws.close()
    ws = null
  }
  connectionStatus.value = 'disconnected'
}

// Reconnect button handler
const reconnect = () => {
  disconnect()
  if (xterm) {
    xterm.clear()
    xterm.write('[Reconnecting...]\r\n')
  }
  connect()
}

// Watch for visibility changes
watch(() => props.visible, (isVisible) => {
  if (isVisible) {
    nextTick(() => {
      initTerminal()
      connect()
    })
  } else {
    disconnect()
  }
})

// Cleanup on unmount
onUnmounted(() => {
  disconnect()
  window.removeEventListener('resize', handleResize)
  if (xterm) {
    xterm.dispose()
    xterm = null
  }
})

onMounted(() => {
  if (props.visible) {
    nextTick(() => {
      initTerminal()
      connect()
    })
  }
})
</script>

<template>
  <Dialog 
    :visible="visible" 
    @hide="$emit('close')" 
    header="VM Console" 
    maximizable
    :style="{ width: '800px', height: '600px' }"
    :modal="true"
  >
    <div class="console-container">
      <div class="console-toolbar">
        <span :class="['status', connectionStatus]">{{ connectionStatus }}</span>
        <div class="toolbar-actions">
          <Button 
            icon="pi pi-refresh" 
            @click="reconnect" 
            :disabled="connectionStatus === 'connecting'"
            v-tooltip="'Reconnect'"
            text
            severity="secondary"
          />
          <Button 
            icon="pi pi-times" 
            @click="$emit('close')"
            v-tooltip="'Close console'"
            text
            severity="secondary"
          />
        </div>
      </div>
      <div ref="terminal" class="terminal"></div>
    </div>
  </Dialog>
</template>

<style scoped>
.console-container {
  display: flex;
  flex-direction: column;
  height: 500px;
  background: #000000;
  border-radius: 4px;
  overflow: hidden;
}

.console-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  background: #1a1a1a;
  border-bottom: 1px solid #333;
}

.status {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 500;
  text-transform: uppercase;
}

.status::before {
  content: '';
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.status.disconnected::before {
  background: #888;
}

.status.disconnected {
  color: #888;
}

.status.connecting::before {
  background: #ffa500;
  animation: pulse 1s infinite;
}

.status.connecting {
  color: #ffa500;
}

.status.connected::before {
  background: #00ff00;
}

.status.connected {
  color: #00ff00;
}

.status.error::before {
  background: #ff0000;
}

.status.error {
  color: #ff0000;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.toolbar-actions {
  display: flex;
  gap: 4px;
}

.terminal {
  flex: 1;
  padding: 8px;
  overflow: hidden;
}

/* Override xterm styles for better integration */
:deep(.xterm) {
  height: 100%;
}

:deep(.xterm .xterm-viewport) {
  background-color: #000000 !important;
}

:deep(.xterm .xterm-screen) {
  background-color: #000000 !important;
}
</style>
