<script lang="ts">
	import type { RunStatus } from '$lib/bff/architectures';

	interface Props {
		status: RunStatus;
		/** Optional aria-label override; defaults to the status text. */
		ariaLabel?: string;
	}

	let { status, ariaLabel }: Props = $props();

	const label: Record<RunStatus, string> = {
		queued: 'Queued',
		running: 'Running',
		succeeded: 'Succeeded',
		partially_failed: 'Partially failed',
		failed: 'Failed',
		cancelled: 'Cancelled'
	};
</script>

<span
	class="badge badge-{status}"
	role="status"
	aria-label={ariaLabel ?? `Run status: ${label[status]}`}
	data-testid="run-status-badge"
	data-status={status}
>
	{label[status]}
</span>

<style>
	.badge {
		display: inline-block;
		padding: 0.15rem 0.6rem;
		border-radius: var(--radius-xs);
		font-size: 12px;
		font-weight: 600;
		letter-spacing: 0.02em;
		color: white;
	}
	.badge-queued { background: #6b7280; }
	.badge-running { background: #1d4ed8; }
	.badge-succeeded { background: #15803d; }
	.badge-partially_failed { background: #b45309; }
	.badge-failed { background: #b91c1c; }
	.badge-cancelled { background: #4b5563; }
</style>
