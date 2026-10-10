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
	} else {
		res.end(EMPTY_LIST);
	}
});

const port = process.env.MOCK_BFF_PORT || 8888;
server.listen(port, () => {
	console.log(`Mock BFF listening on :${port}`);
});
