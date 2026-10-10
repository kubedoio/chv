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
// Guest collector fixtures (campaign #602 G4): dimensioned fs/process
// series and check results for the fixture VM (vm-1) the VM detail e2e
// navigates to. Shapes are byte-compatible with MonitoringCurrentSample
// — integer values travel as decimal strings. Every other target gets
// the honest empty responses above, never vm-1's data.
const MOCK_NOW = Date.now();
const GUEST_CURRENT_VM_1_JSON = JSON.stringify({
	schema_version: 1,
	target_kind: 'vm',
	target_id: 'vm-1',
	generated_at_ms: MOCK_NOW,
	samples: [
		{
			metric_id: 'vm.guest.fs.total_bytes',
			source: 'guest_agent',
			dimensions: { mount_id: 'ext4:/' },
			kind: 'gauge',
			unit: 'bytes',
			observed_at_ms: MOCK_NOW - 5_000,
			received_at_ms: MOCK_NOW - 4_000,
			quality: 'valid',
			stale: false,
			integer_value: '536870912000'
		},
		{
			metric_id: 'vm.guest.fs.available_bytes',
			source: 'guest_agent',
			dimensions: { mount_id: 'ext4:/' },
			kind: 'gauge',
			unit: 'bytes',
			observed_at_ms: MOCK_NOW - 5_000,
			received_at_ms: MOCK_NOW - 4_000,
			quality: 'valid',
			stale: false,
			integer_value: '268435456000'
		},
		{
			metric_id: 'vm.guest.fs.inodes_utilization_ratio',
			source: 'guest_agent',
			dimensions: { mount_id: 'ext4:/' },
			kind: 'gauge',
			unit: 'ratio',
			observed_at_ms: MOCK_NOW - 5_000,
			received_at_ms: MOCK_NOW - 4_000,
			quality: 'valid',
			stale: false,
			value: 0.041
		},
		{
			metric_id: 'vm.guest.fs.read_only',
			source: 'guest_agent',
			dimensions: { mount_id: 'ext4:/' },
			kind: 'state',
			unit: 'boolean',
			observed_at_ms: MOCK_NOW - 5_000,
			received_at_ms: MOCK_NOW - 4_000,
			quality: 'valid',
			stale: false,
			integer_value: '0'
		},
		{
			metric_id: 'vm.guest.fs.total_bytes',
			source: 'guest_agent',
			dimensions: { mount_id: 'xfs:/var' },
			kind: 'gauge',
			unit: 'bytes',
			observed_at_ms: MOCK_NOW - 400_000,
			received_at_ms: MOCK_NOW - 399_000,
			quality: 'valid',
			stale: true,
			integer_value: '10737418240'
		},
		{
			metric_id: 'vm.guest.fs.available_bytes',
			source: 'guest_agent',
			dimensions: { mount_id: 'xfs:/var' },
			kind: 'gauge',
			unit: 'bytes',
			observed_at_ms: MOCK_NOW - 400_000,
			received_at_ms: MOCK_NOW - 399_000,
			quality: 'valid',
			stale: true,
			integer_value: '536870912'
		},
		{
			metric_id: 'vm.guest.fs.inodes_utilization_ratio',
			source: 'guest_agent',
			dimensions: { mount_id: 'xfs:/var' },
			kind: 'gauge',
			unit: 'ratio',
			observed_at_ms: MOCK_NOW - 400_000,
			received_at_ms: MOCK_NOW - 399_000,
			quality: 'valid',
			stale: true,
			value: 0.87
		},
		{
			metric_id: 'vm.guest.fs.read_only',
			source: 'guest_agent',
			dimensions: { mount_id: 'xfs:/var' },
			kind: 'state',
			unit: 'boolean',
			observed_at_ms: MOCK_NOW - 400_000,
			received_at_ms: MOCK_NOW - 399_000,
			quality: 'valid',
			stale: true,
			integer_value: '1'
		},
		{
			metric_id: 'vm.guest.process.count',
			source: 'guest_agent',
			dimensions: { process_selector: 'nginx' },
			kind: 'gauge',
			unit: 'count',
			observed_at_ms: MOCK_NOW - 5_000,
			received_at_ms: MOCK_NOW - 4_000,
			quality: 'valid',
			stale: false,
			value: 4
		},
		{
			metric_id: 'vm.guest.process.cpu_utilization_ratio',
			source: 'guest_agent',
			dimensions: { process_selector: 'nginx' },
			kind: 'gauge',
			unit: 'ratio',
			observed_at_ms: MOCK_NOW - 5_000,
			received_at_ms: MOCK_NOW - 4_000,
			quality: 'valid',
			stale: false,
			value: 0.12
		},
		{
			metric_id: 'vm.guest.process.rss_bytes',
			source: 'guest_agent',
			dimensions: { process_selector: 'nginx' },
			kind: 'gauge',
			unit: 'bytes',
			observed_at_ms: MOCK_NOW - 5_000,
			received_at_ms: MOCK_NOW - 4_000,
			quality: 'valid',
			stale: false,
			integer_value: '134217728'
		},
		{
			metric_id: 'vm.guest.process.count',
			source: 'guest_agent',
			dimensions: { process_selector: 'postgres' },
			kind: 'gauge',
			unit: 'count',
			observed_at_ms: MOCK_NOW - 300_000,
			received_at_ms: MOCK_NOW - 299_000,
			quality: 'valid',
			stale: true,
			value: 1
		},
		{
			metric_id: 'vm.guest.process.cpu_utilization_ratio',
			source: 'guest_agent',
			dimensions: { process_selector: 'postgres' },
			kind: 'gauge',
			unit: 'ratio',
			observed_at_ms: MOCK_NOW - 300_000,
			received_at_ms: MOCK_NOW - 299_000,
			quality: 'valid',
			stale: true,
			value: 0.03
		},
		{
			metric_id: 'vm.guest.process.rss_bytes',
			source: 'guest_agent',
			dimensions: { process_selector: 'postgres' },
			kind: 'gauge',
			unit: 'bytes',
			observed_at_ms: MOCK_NOW - 300_000,
			received_at_ms: MOCK_NOW - 299_000,
			quality: 'valid',
			stale: true,
			integer_value: '805306368'
		}
	]
});
// Guest check results (query/alerts contract v1, #602 G4): all four
// statuses, service/http/plugin kinds, one stale record. An empty
// checks array is a valid state (no checks configured).
const MONITORING_CHECKS_VM_1 = JSON.stringify({
	schema_version: 1,
	target_kind: 'vm',
	target_id: 'vm-1',
	generated_at_ms: MOCK_NOW,
	checks: [
		{
			check_id: 'service:nginx.service',
			service_key: 'nginx.service',
			status: 'ok',
			summary: 'active (running)',
			observed_at_ms: MOCK_NOW - 9_000,
			received_at_ms: MOCK_NOW - 8_000,
			agent_id: 'agent-vm-1',
			stale: false
		},
		{
			check_id: 'service:postgresql.service',
			service_key: 'postgresql.service',
			status: 'critical',
			summary: 'failed (Result: exit-code)',
			observed_at_ms: MOCK_NOW - 12_000,
			received_at_ms: MOCK_NOW - 11_000,
			agent_id: 'agent-vm-1',
			stale: false
		},
		{
			check_id: 'http:public-api',
			service_key: 'public-api',
			status: 'warning',
			summary: '500 Internal Server Error in 230ms',
			observed_at_ms: MOCK_NOW - 15_000,
			received_at_ms: MOCK_NOW - 14_000,
			agent_id: 'agent-vm-1',
			stale: false
		},
		{
			check_id: 'plugin:backup-freshness',
			service_key: 'backup-freshness',
			status: 'unknown',
			summary: 'plugin did not report a result',
			observed_at_ms: MOCK_NOW - 400_000,
			received_at_ms: MOCK_NOW - 399_000,
			agent_id: 'agent-vm-1',
			stale: true
		}
	]
});
const MONITORING_CHECKS_EMPTY = JSON.stringify({
	schema_version: 1,
	target_kind: 'vm',
	target_id: 'unknown',
	generated_at_ms: MOCK_NOW,
	checks: []
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
// Native alerting (query/alerts contract v1, #602 PR-6): honest empty
// surfaces — no incidents, no rules, no deliveries. Shapes are
// byte-compatible with the BFF's incident/rule/delivery renderers.
const ALERT_INCIDENTS = JSON.stringify({ incidents: [], total: 0 });
const ALERT_RULES = JSON.stringify({ rules: [], total: 0 });
const ALERT_DELIVERIES = JSON.stringify({ deliveries: [] });

/** Read a JSON request body; an unparseable body reads as `{}`. */
function readJsonBody(req) {
	return new Promise((resolve) => {
		let raw = '';
		req.on('data', (chunk) => {
			raw += chunk;
		});
		req.on('end', () => {
			try {
				resolve(JSON.parse(raw || '{}'));
			} catch {
				resolve({});
			}
		});
	});
}

const server = http.createServer(async (req, res) => {
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
		// Keyed by target id: only the fixture VM gets guest samples —
		// every other target gets the honest empty store.
		const body = await readJsonBody(req);
		res.end(body.target_id === 'vm-1' ? GUEST_CURRENT_VM_1_JSON : MONITORING_CURRENT);
	} else if (path === '/v1/monitoring/checks') {
		const body = await readJsonBody(req);
		res.end(body.target_id === 'vm-1' ? MONITORING_CHECKS_VM_1 : MONITORING_CHECKS_EMPTY);
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
	} else if (path === '/v1/monitoring/alerts') {
		res.end(ALERT_INCIDENTS);
	} else if (path === '/v1/monitoring/alert-rules') {
		res.end(ALERT_RULES);
	} else if (path === '/v1/monitoring/notifications/deliveries') {
		res.end(ALERT_DELIVERIES);
	} else {
		res.end(EMPTY_LIST);
	}
});

const port = process.env.MOCK_BFF_PORT || 8888;
server.listen(port, () => {
	console.log(`Mock BFF listening on :${port}`);
});
