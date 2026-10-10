import http from 'node:http';

const EMPTY_LIST = JSON.stringify({ items: [], page: { page: 1, page_size: 50, total_items: 0 }, filters: { applied: {} } });
const EMPTY_OVERVIEW = JSON.stringify({
	clusters_total: 0, clusters_healthy: 0, clusters_degraded: 0,
	nodes_total: 0, nodes_degraded: 0, vms_running: 0, vms_total: 0,
	active_tasks: 0, unresolved_alerts: 0, maintenance_nodes: 0,
	capacity_hotspots: 0, cpu_usage_percent: 0, memory_usage_percent: 0,
	storage_usage_percent: 0, alerts: [], recent_tasks: []
});

// Monitoring v1 read API (query/alerts contract shapes): an empty but
// HONEST store — no series, absence reasons, never zeros. Without
// these routes the catch-all below would answer every monitoring call
// with the node-list shape and crash the UI's typed parsers.
const MONITORING_CATALOG = JSON.stringify({
	schema_version: 1,
	metrics: [],
	qualities: ['valid', 'insufficient_samples', 'unsupported', 'unavailable', 'invalid', 'stale'],
	series_reasons: ['unsupported', 'not_collected', 'no_history', 'stale']
});
const MONITORING_HEALTH = JSON.stringify({
	schema_version: 1,
	available: true,
	degraded_reason: null,
	last_ingest_at_ms: null,
	last_maintenance_at_ms: null,
	accepted_batches: 0,
	duplicate_batches: 0,
	rejected_batches: 0,
	unavailable_batches: 0,
	headroom_bytes: null,
	headroom_probe_failed: false,
	raw_samples: 0,
	generated_at_ms: 0
});
const MONITORING_HISTORY = JSON.stringify({
	schema_version: 1,
	target_kind: 'node',
	target_id: 'node-1',
	series: [],
	generated_at_ms: 0,
	truncated: false
});
const MONITORING_CURRENT = JSON.stringify({
	schema_version: 1,
	target_kind: 'node',
	target_id: 'node-1',
	samples: [],
	generated_at_ms: 0
});
const MONITORING_OVERVIEW = JSON.stringify({
	schema_version: 1,
	target_kind: 'node',
	targets: [],
	generated_at_ms: 0
});
// Guest monitoring agent administration (ADR-026 G3): an empty but
// honest inventory — "no agents enrolled", never a shape the UI would
// misread as data. The operator actions answer with their contract
// shapes so flows can be walked end-to-end.
const MONITORING_AGENTS = JSON.stringify({
	schema_version: 1,
	agents: [],
	generated_at_ms: 0,
	truncated: false
});
const MONITORING_AGENT_CLAIM = JSON.stringify({
	schema_version: 1,
	vm_id: 'vm-1',
	claim_token: 'chvm_mock_claim_token_not_a_real_secret',
	expires_at_ms: Date.now() + 600_000,
	server_url: 'https://manager.example:8443',
	ca_fingerprint: '00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff'
});
const MONITORING_AGENT_ACTION = JSON.stringify({
	schema_version: 1,
	agent_id: 'agent-1'
});

const server = http.createServer((req, res) => {
	res.setHeader('Content-Type', 'application/json');
	res.setHeader('Access-Control-Allow-Origin', '*');

	const path = (req.url || '').split('?')[0];
	if (path === '/v1/overview') {
		res.end(EMPTY_OVERVIEW);
	} else if (path === '/v1/monitoring/catalog') {
		res.end(MONITORING_CATALOG);
	} else if (path === '/v1/monitoring/health') {
		res.end(MONITORING_HEALTH);
	} else if (path === '/v1/monitoring/history') {
		res.end(MONITORING_HISTORY);
	} else if (path === '/v1/monitoring/current') {
		res.end(MONITORING_CURRENT);
	} else if (path === '/v1/monitoring/overview') {
		res.end(MONITORING_OVERVIEW);
	} else if (path === '/v1/monitoring/agents') {
		res.end(MONITORING_AGENTS);
	} else if (path === '/v1/monitoring/agents/claim') {
		res.end(MONITORING_AGENT_CLAIM);
	} else if (
		path === '/v1/monitoring/agents/revoke' ||
		path === '/v1/monitoring/agents/rotate' ||
		path === '/v1/monitoring/agents/reset'
	) {
		res.end(MONITORING_AGENT_ACTION);
	} else {
		res.end(EMPTY_LIST);
	}
});

const port = process.env.MOCK_BFF_PORT || 8888;
server.listen(port, () => {
	console.log(`Mock BFF listening on :${port}`);
});
