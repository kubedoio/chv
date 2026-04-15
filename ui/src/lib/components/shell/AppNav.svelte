<script lang="ts">
	import { page } from '$app/stores';
	import { navigationItems } from '$lib/shell/app-shell';

	function isActive(href: string, pathname: string): boolean {
		if (href === '/') {
			return pathname === '/';
		}

		return pathname === href || pathname.startsWith(`${href}/`);
	}
</script>

<nav class="app-nav" aria-label="Primary">
	<div class="app-nav__brand">
		<div class="app-nav__brand-mark">CHV</div>
		<div>
			<div class="app-nav__brand-title">Control Plane</div>
			<div class="app-nav__brand-subtitle">Starter bundle shell</div>
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
		<div class="app-nav__footer-label">Design intent</div>
		<p>Restraint, legibility, and task transparency before density or decoration.</p>
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

	.app-nav__brand-title,
	.app-nav__brand-subtitle {
		line-height: 1.2;
	}

	.app-nav__brand-title {
		font-weight: 600;
		color: var(--shell-text);
	}

	.app-nav__brand-subtitle,
	.app-nav__footer-label {
		font-size: 0.74rem;
		letter-spacing: 0.1em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	.app-nav__links {
		display: grid;
		gap: 0.35rem;
	}

	.app-nav__link {
		display: flex;
		align-items: center;
		gap: 0.8rem;
		border: 1px solid transparent;
		border-radius: 0.95rem;
		padding: 0.8rem 0.9rem;
		font-size: 0.95rem;
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
		gap: 0.35rem;
		border-top: 1px solid var(--shell-line);
		padding-top: 1rem;
	}

	.app-nav__footer p {
		font-size: 0.88rem;
		line-height: 1.5;
		color: var(--shell-text-muted);
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
