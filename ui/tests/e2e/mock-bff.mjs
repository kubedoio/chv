import http from 'node:http';

const EMPTY_LIST = JSON.stringify({ items: [], page: { page: 1, page_size: 50, total_items: 0 }, filters: { applied: {} } });
const EMPTY_OVERVIEW = JSON.stringify({
	clusters_total: 0, clusters_healthy: 0, clusters_degraded: 0,
	nodes_total: 0, nodes_degraded: 0, vms_running: 0, vms_total: 0,
	active_tasks: 0, unresolved_alerts: 0, maintenance_nodes: 0,
	capacity_hotspots: 0, cpu_usage_percent: 0, memory_usage_percent: 0,
	storage_usage_percent: 0, alerts: [], recent_tasks: []
});

const server = http.createServer((req, res) => {
	res.setHeader('Content-Type', 'application/json');
	res.setHeader('Access-Control-Allow-Origin', '*');

	if (req.url === '/v1/overview') {
		res.end(EMPTY_OVERVIEW);
	} else {
		res.end(EMPTY_LIST);
	}
});

const port = process.env.MOCK_BFF_PORT || 8888;
server.listen(port, () => {
	console.log(`Mock BFF listening on :${port}`);
});
