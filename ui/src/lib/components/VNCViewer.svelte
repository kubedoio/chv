<script lang="ts">
  interface Props {
    wsUrl: string;
    onClose?: () => void;
    fullscreen?: boolean;
  }
  let { wsUrl, onClose, fullscreen = false }: Props = $props();
</script>

<div class="vnc-container border border-line rounded bg-black" class:vnc-fullscreen={fullscreen}>
  <div class="vnc-header flex justify-between items-center px-3 py-2 bg-gray-800 border-b border-gray-700">
    <div class="flex items-center gap-2">
      <div class="w-3 h-3 rounded-full bg-green-500"></div>
      <span class="text-xs font-mono text-white">VNC Connected</span>
    </div>
    <button onclick={onClose} class="text-xs text-gray-400 hover:text-white">Close</button>
  </div>
  <div class="vnc-viewport flex-1 flex items-center justify-center bg-black relative">
    <!-- noVNC canvas will be mounted here -->
    <div id="vnc-canvas-container" class="w-full h-full flex items-center justify-center">
      <div class="text-center text-gray-400">
        <p class="mb-2">VNC Console</p>
        <p class="text-xs mb-4">Connecting to {wsUrl}...</p>
        <div class="text-xs text-left max-w-md mx-auto space-y-1">
          <p>VNC console requires a VNC client. Options:</p>
          <ul class="list-disc list-inside mt-2 space-y-1">
            <li>Use a desktop VNC client (TigerVNC, RealVNC, etc.)</li>
            <li>Connect via the VM's IP with a VNC viewer</li>
            <li>Use the PTY console for text-based access</li>
          </ul>
        </div>
        <a 
          href={wsUrl.replace('ws://', 'http://').replace('wss://', 'https://')} 
          target="_blank"
          class="mt-4 inline-block px-4 py-2 bg-blue-600 text-white text-xs rounded hover:bg-blue-700"
        >
          Open VNC WebSocket
        </a>
      </div>
    </div>
  </div>
</div>

<style>
  .vnc-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 400px;
  }
  .vnc-fullscreen {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 100;
  }
  .vnc-viewport {
    min-height: 300px;
  }
</style>
