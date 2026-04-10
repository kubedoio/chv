function getDefaultNode() {
  return {
    id: "local",
    name: "Local Node",
    hostname: "localhost",
    ip_address: "127.0.0.1",
    status: "online",
    is_local: true,
    capabilities: void 0,
    last_seen_at: (/* @__PURE__ */ new Date()).toISOString(),
    created_at: (/* @__PURE__ */ new Date()).toISOString(),
    updated_at: (/* @__PURE__ */ new Date()).toISOString()
  };
}
export {
  getDefaultNode as g
};
