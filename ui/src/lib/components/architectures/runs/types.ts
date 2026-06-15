/**
 * Operation progress shape consumed by `OperationProgressRow.svelte` and the
 * per-run page. The orchestrator writes `result_json` containing
 * `{ operations: OperationProgress[] }` on terminal transitions; the page
 * parses defensively so a queued run still renders something useful.
 */
export interface OperationProgress {
	resource_ref: string;
	action: string;
	status: 'pending' | 'running' | 'succeeded' | 'failed' | 'cancelled';
	error_message?: string | null;
	operation_id?: string | null;
}
