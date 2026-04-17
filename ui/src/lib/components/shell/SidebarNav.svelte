<script lang="ts">
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { LogOut } from 'lucide-svelte';
	import { navigationItems } from '$lib/shell/app-shell';
	import { clearToken, createAPIClient } from '$lib/api/client';

	function isActive(href: string, pathname: string): boolean {
		if (href === '/') {
			return pathname === '/';
		}

		return pathname === href || pathname.startsWith(`${href}/`);
	}

	async function handleLogout() {
		try {
			await createAPIClient().logout();
		} catch {
			// Best-effort remote logout; local token removal is authoritative for WebUI.
		} finally {
			clearToken();
			goto('/login');
		}
	}
</script>

<nav class="app-nav" aria-label="Primary">
	<div class="app-nav__brand">
		<div class="app-nav__brand-mark">CHV</div>
		<div>
			<div class="app-nav__brand-title">Control Plane</div>
		</div>
	</div>

	<div class="app-nav__links">
		{#each navigationItems as item}
			<a
				href={item.href}
				class:app-nav__link--active={isActive(item.href, $page.url.pathname)}
				class="app-nav__link"
				aria-current={isActive(item.href, $page.url.pathname) ? 'page' : undefined}
			>
				<item.icon size={17}></item.icon>
				<span>{item.label}</span>
			</a>
		{/each}
	</div>

	<div class="app-nav__footer">
		<button type="button" class="app-nav__logout" onclick={handleLogout} aria-label="Log out">
			<LogOut size={15}></LogOut>
			<span>Log out</span>
		</button>
	</div>
</nav>

<style>
	.app-nav {
		display: grid;
		gap: 1.25rem;
	}

	.app-nav__brand {
		display: flex;
		align-items: center;
		gap: 0.9rem;
	}

	.app-nav__brand-mark {
		display: grid;
		place-items: center;
		width: 2.6rem;
		height: 2.6rem;
		border-radius: 0.9rem;
		background: var(--shell-accent);
		color: #fffaf3;
		font-size: 0.86rem;
		font-weight: 700;
		letter-spacing: 0.08em;
	}

	.app-nav__brand-title {
		line-height: 1.2;
	}

	.app-nav__brand-title {
		font-weight: 600;
		color: var(--shell-text);
	}

	.app-nav__footer-label {
		font-size: 0.74rem;
		letter-spacing: 0.1em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	.app-nav__links {
		display: grid;
		gap: 0.15rem;
	}

	.app-nav__link {
		display: flex;
		align-items: center;
		gap: 0.6rem;
		border: 1px solid transparent;
		border-radius: 0.5rem;
		padding: 0.5rem 0.65rem;
		font-size: 0.85rem;
		font-weight: 500;
		color: var(--shell-text-secondary);
		text-decoration: none;
		transition:
			background-color 140ms ease,
			border-color 140ms ease,
			color 140ms ease,
			transform 140ms ease;
	}

	.app-nav__link:hover {
		background: var(--shell-surface);
		border-color: var(--shell-line);
		color: var(--shell-text);
		transform: translateX(2px);
	}

	.app-nav__link--active {
		background: var(--shell-surface);
		border-color: var(--shell-line-strong);
		color: var(--shell-text);
	}

	.app-nav__footer {
		display: grid;
		border-top: 1px solid var(--shell-line);
		padding-top: 0.75rem;
	}

	.app-nav__logout {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
		width: fit-content;
		border: 1px solid var(--shell-line);
		border-radius: 0.75rem;
		padding: 0.5rem 0.65rem;
		background: var(--shell-surface);
		color: var(--shell-text-secondary);
		font-size: 0.82rem;
		font-weight: 600;
		cursor: pointer;
		transition:
			background-color 140ms ease,
			border-color 140ms ease,
			color 140ms ease;
	}

	.app-nav__logout:hover {
		background: color-mix(in srgb, var(--shell-surface) 72%, var(--status-failed-bg) 28%);
		border-color: color-mix(in srgb, var(--shell-line) 60%, var(--status-failed-border) 40%);
		color: color-mix(in srgb, var(--shell-text) 62%, var(--status-failed-text) 38%);
	}

	@media (max-width: 960px) {
		.app-nav__links {
			grid-auto-flow: column;
			grid-auto-columns: minmax(10rem, 1fr);
			overflow-x: auto;
			padding-bottom: 0.35rem;
		}

		.app-nav__footer {
			display: none;
		}
	}
</style>
