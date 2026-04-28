-- Add agent_ws_address for multi-node WebSocket console routing

ALTER TABLE nodes ADD COLUMN agent_ws_address TEXT;
