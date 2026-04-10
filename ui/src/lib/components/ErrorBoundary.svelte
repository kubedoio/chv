<script lang="ts">
	import type { Snippet } from 'svelte';
	import { browser } from '$app/environment';
	import { AlertTriangle, RefreshCw, Home } from 'lucide-svelte';
	import Button from './primitives/Button.svelte';

	interface Props {
		children: Snippet;
		fallback?: Snippet<[Error, () => void]>;
		onError?: (error: Error, errorInfo: string) => void;
	}

	let { children, fallback, onError }: Props = $props();

	let hasError = $state(false);
	let error = $state<Error | null>(null);
	let errorInfo = $state('');
	let errorId = $state('');

	// Generate unique error ID for reporting
	function generateErrorId(): string {
		return `err-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;
	}

	function resetError() {
		hasError = false;
		error = null;
		errorInfo = '';
		errorId = '';
	}

	function handleError(err: unknown, reset: () => void) {
		hasError = true;
		error = err instanceof Error ? err : new Error(String(err));
		errorId = generateErrorId();

		if (onError && error) {
			onError(error, errorInfo);
		}

		// Log to console in development
		if (browser && import.meta.env.DEV) {
			console.error('ErrorBoundary caught error:', err);
		}
	}

	function handleReload() {
		if (browser) {
			window.location.reload();
		}
	}

	function handleGoHome() {
		if (browser) {
			window.location.href = '/';
		}
	}

	// Default error fallback snippet
	function defaultFallback(err: Error, reset: () => void) {
		return `
			<div 
				class="error-boundary"
				role="alert"
				aria-live="assertive"
			>
				<div class="error-content">
					<div class="error-icon">
						<AlertTriangle size={48} aria-hidden="true" />
					</div>
					
					<h1 class="error-title">Something went wrong</h1>
					
					<p class="error-message">
						${err?.message || 'An unexpected error occurred'}
					</p>

					${errorId ? `<p class="error-id">Error ID: <code>${errorId}</code></p>` : ''}

					<div class="error-actions">
						<Button variant="primary" onclick={reset}>
							<RefreshCw size={16} aria-hidden="true" />
							Try Again
						</Button>
						
						<Button variant="secondary" onclick={handleReload}>
							Reload Page
						</Button>
						
						<Button variant="ghost" onclick={handleGoHome}>
							<Home size={16} aria-hidden="true" />
							Go Home
						</Button>
					</div>
				</div>
			</div>
		`;
	}
</script>

{#if hasError}
	{#if fallback}
		{@render fallback(error!, resetError)}
	{:else}
		<!-- Default error UI -->
		<div 
			class="error-boundary"
			role="alert"
			aria-live="assertive"
		>
			<div class="error-content">
				<div class="error-icon">
					<AlertTriangle size={48} aria-hidden="true" />
				</div>
				
				<h1 class="error-title">Something went wrong</h1>
				
				<p class="error-message">
					{error?.message || 'An unexpected error occurred'}
				</p>

				{#if errorId}
					<p class="error-id">
						Error ID: <code>{errorId}</code>
					</p>
				{/if}

				<div class="error-actions">
					<Button variant="primary" onclick={resetError}>
						<RefreshCw size={16} aria-hidden="true" />
						Try Again
					</Button>
					
					<Button variant="secondary" onclick={handleReload}>
						Reload Page
					</Button>
					
					<Button variant="ghost" onclick={handleGoHome}>
						<Home size={16} aria-hidden="true" />
						Go Home
					</Button>
				</div>
			</div>
		</div>
	{/if}
{:else}
	<svelte:boundary onerror={handleError}>
		{@render children()}
	</svelte:boundary>
{/if}

<style>
	.error-boundary {
		min-height: 100vh;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 2rem;
		background: var(--color-neutral-50);
	}

	.error-content {
		max-width: 500px;
		text-align: center;
	}

	.error-icon {
		color: var(--color-danger);
		margin-bottom: 1.5rem;
		display: flex;
		justify-content: center;
	}

	.error-title {
		font-size: var(--text-2xl);
		font-weight: 600;
		color: var(--color-neutral-900);
		margin-bottom: 0.75rem;
	}

	.error-message {
		font-size: var(--text-base);
		color: var(--color-neutral-600);
		margin-bottom: 1rem;
	}

	.error-id {
		font-size: var(--text-xs);
		color: var(--color-neutral-400);
		margin-bottom: 1.5rem;
	}

	.error-id code {
		background: var(--color-neutral-100);
		padding: 0.125rem 0.375rem;
		border-radius: 4px;
		font-family: var(--font-mono);
	}

	.error-actions {
		display: flex;
		gap: 0.75rem;
		justify-content: center;
		flex-wrap: wrap;
	}

	@media (prefers-reduced-motion: reduce) {
		.error-boundary {
			animation: none;
		}
	}
</style>
