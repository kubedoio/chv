<script lang="ts">
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { browser } from '$app/environment';
	import { slide, fade } from 'svelte/transition';
	import { 
		Menu, 
		X, 
		Database,
		LayoutGrid,
		Server,
		Image as ImageIcon,
		HardDrive,
		Network,
		Settings,
		LogOut
	} from 'lucide-svelte';
	import { clearToken } from '$lib/api/client';

	// Props
	interface Props {
		nodes?: import('$lib/api/nodes').Node[];
		userName?: string;
		userEmail?: string;
	}

	let { 
		nodes = [],
		userName = 'Administrator',
		userEmail = 'admin@chv.local'
	}: Props = $props();

	let isOpen = $state(false);
	let expandedNodes = $state<Set<string>>(new Set(['datacenter']));

	let currentPath = $derived($page.url.pathname);

	const navItems = [
		{ id: 'overview', label: 'Overview', icon: LayoutGrid, href: '/' },
		{ id: 'global-images', label: 'Images', icon: ImageIcon, href: '/images' },
		{ id: 'global-storage', label: 'Storage', icon: HardDrive, href: '/storage' },
		{ id: 'global-networks', label: 'Networks', icon: Network, href: '/networks' },
		{ id: 'settings', label: 'Settings', icon: Settings, href: '/settings' },
	];

	function toggleMenu() {
		isOpen = !isOpen;
		// Prevent body scroll when menu is open
		if (browser) {
			document.body.style.overflow = isOpen ? 'hidden' : '';
		}
	}

	function closeMenu() {
		isOpen = false;
		if (browser) {
			document.body.style.overflow = '';
		}
	}

	function handleNavClick(href: string) {
		goto(href);
		closeMenu();
	}

	function toggleNode(nodeId: string) {
		if (expandedNodes.has(nodeId)) {
			expandedNodes.delete(nodeId);
		} else {
			expandedNodes.add(nodeId);
		}
		expandedNodes = expandedNodes;
	}

	function isActive(href: string): boolean {
		if (href === '/') return currentPath === '/';
		return currentPath.startsWith(href);
	}

	function handleLogout() {
		clearToken();
		goto('/login');
		closeMenu();
	}
</script>

<!-- Mobile Header -->
<header class="mobile-header">
	<div class="header-content">
		<!-- Logo -->
		<div class="logo">
			<div class="logo-icon">
				<Database size={20} aria-hidden="true" />
			</div>
			<span class="logo-text">CHV</span>
		</div>

		<!-- Menu Button -->
		<button
			type="button"
			class="menu-button"
			onclick={toggleMenu}
			aria-expanded={isOpen}
			aria-controls="mobile-nav-menu"
			aria-label={isOpen ? 'Close menu' : 'Open menu'}
		>
			{#if isOpen}
				<X size={24} aria-hidden="true" />
			{:else}
				<Menu size={24} aria-hidden="true" />
			{/if}
		</button>
	</div>
</header>

<!-- Mobile Menu Overlay -->
{#if isOpen}
	<!-- Backdrop -->
	<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
	<div
		class="menu-backdrop"
		transition:fade={{ duration: 200 }}
		onclick={closeMenu}
		aria-hidden="true"
	></div>

	<!-- Menu Panel -->
	<nav
		id="mobile-nav-menu"
		class="menu-panel"
		transition:slide={{ duration: 200, axis: 'x' }}
		aria-label="Mobile navigation"
	>
		<div class="menu-header">
			<div class="logo">
				<div class="logo-icon">
					<Database size={20} aria-hidden="true" />
				</div>
				<div>
					<div class="logo-text">CHV Manager</div>
					<div class="logo-subtitle">Virtualization Platform</div>
				</div>
			</div>
		</div>

		<div class="menu-content">
			<!-- Main Navigation -->
			<ul class="nav-list" role="menubar">
				{#each navItems as item}
					<li role="none">
						<a
							href={item.href}
							role="menuitem"
							class="nav-item"
							class:active={isActive(item.href)}
							aria-current={isActive(item.href) ? 'page' : undefined}
							onclick={(e) => {
								e.preventDefault();
								handleNavClick(item.href);
							}}
						>
							<item.icon size={20} aria-hidden="true" />
							<span>{item.label}</span>
						</a>
					</li>
				{/each}
			</ul>

			<!-- Nodes Section -->
			{#if nodes.length > 0}
				<div class="section-divider" role="separator"></div>
				
				<div class="section-title">Nodes</div>
				
				<ul class="nav-list" role="menubar">
					{#each nodes as node}
						<li role="none">
							<a
								href={`/nodes/${node.id}`}
								role="menuitem"
								class="nav-item"
								class:active={isActive(`/nodes/${node.id}`)}
								aria-current={isActive(`/nodes/${node.id}`) ? 'page' : undefined}
								onclick={(e) => {
									e.preventDefault();
									handleNavClick(`/nodes/${node.id}`);
								}}
							>
								<Server size={20} aria-hidden="true" />
								<span>{node.name}</span>
								<span class="status-indicator" class:online={node.status === 'online'}></span>
							</a>
						</li>
					{/each}
				</ul>
			{/if}
		</div>

		<!-- User Section -->
		<div class="menu-footer">
			<div class="user-info">
				<div class="user-avatar">
					{userName.slice(0, 2).toUpperCase()}
				</div>
				<div class="user-details">
					<div class="user-name">{userName}</div>
					<div class="user-email">{userEmail}</div>
				</div>
			</div>
			
			<button
				type="button"
				class="logout-button"
				onclick={handleLogout}
				aria-label="Sign out"
			>
				<LogOut size={18} aria-hidden="true" />
			</button>
		</div>
	</nav>
{/if}

<style>
	.mobile-header {
		display: none;
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		z-index: 50;
		height: 56px;
		background: var(--bg-sidebar);
		border-bottom: 1px solid rgba(255, 255, 255, 0.08);
	}

	.header-content {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 100%;
		padding: 0 1rem;
	}

	.logo {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}

	.logo-icon {
		width: 32px;
		height: 32px;
		border-radius: 8px;
		background: var(--color-primary);
		display: flex;
		align-items: center;
		justify-content: center;
		color: white;
	}

	.logo-text {
		font-size: 1.125rem;
		font-weight: 600;
		color: white;
	}

	.logo-subtitle {
		font-size: 0.625rem;
		color: var(--color-neutral-400);
	}

	.menu-button {
		width: 44px;
		height: 44px;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 8px;
		color: var(--color-neutral-400);
		background: transparent;
		border: none;
		cursor: pointer;
		transition: all 0.15s ease;
	}

	.menu-button:hover {
		background: rgba(255, 255, 255, 0.1);
		color: var(--color-neutral-50);
	}

	.menu-button:focus-visible {
		outline: 2px solid var(--color-primary);
		outline-offset: -2px;
	}

	.menu-backdrop {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.5);
		z-index: 40;
	}

	.menu-panel {
		position: fixed;
		top: 0;
		left: 0;
		bottom: 0;
		width: 280px;
		max-width: 80vw;
		background: var(--bg-sidebar);
		z-index: 41;
		display: flex;
		flex-direction: column;
		border-right: 1px solid rgba(255, 255, 255, 0.08);
	}

	.menu-header {
		padding: 1rem;
		border-bottom: 1px solid rgba(255, 255, 255, 0.08);
		background: rgba(0, 0, 0, 0.14);
	}

	.menu-content {
		flex: 1;
		overflow-y: auto;
		padding: 0.5rem 0;
	}

	.nav-list {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.nav-item {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.75rem 1rem;
		color: var(--color-neutral-300);
		text-decoration: none;
		font-size: 0.875rem;
		transition: all 0.15s ease;
		position: relative;
	}

	.nav-item:hover {
		background: rgba(255, 255, 255, 0.05);
		color: var(--color-neutral-50);
	}

	.nav-item.active {
		background: rgba(var(--color-primary-rgb), 0.16);
		color: #fff7ef;
	}

	.nav-item.active::before {
		content: '';
		position: absolute;
		left: 0;
		top: 50%;
		transform: translateY(-50%);
		width: 3px;
		height: 20px;
		background: var(--color-primary);
		border-radius: 0 2px 2px 0;
	}

	.nav-item:focus-visible {
		outline: 2px solid var(--color-primary);
		outline-offset: -2px;
	}

	.section-divider {
		height: 1px;
		background: rgba(255, 255, 255, 0.08);
		margin: 0.5rem 1rem;
	}

	.section-title {
		padding: 0.5rem 1rem;
		font-size: 0.625rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.1em;
		color: var(--color-neutral-400);
	}

	.status-indicator {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: #ef4444;
		margin-left: auto;
	}

	.status-indicator.online {
		background: var(--color-success);
	}

	.menu-footer {
		padding: 1rem;
		border-top: 1px solid rgba(255, 255, 255, 0.08);
		background: rgba(0, 0, 0, 0.14);
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.user-info {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}

	.user-avatar {
		width: 36px;
		height: 36px;
		border-radius: 50%;
		background: rgba(255, 255, 255, 0.12);
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 0.75rem;
		font-weight: 600;
		color: white;
	}

	.user-details {
		min-width: 0;
	}

	.user-name {
		font-size: 0.875rem;
		font-weight: 500;
		color: white;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.user-email {
		font-size: 0.75rem;
		color: var(--color-neutral-400);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.logout-button {
		width: 36px;
		height: 36px;
		border-radius: 8px;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--color-neutral-400);
		background: transparent;
		border: none;
		cursor: pointer;
		transition: all 0.15s ease;
	}

	.logout-button:hover {
		background: rgba(var(--color-danger-rgb), 0.12);
		color: var(--color-danger);
	}

	.logout-button:focus-visible {
		outline: 2px solid var(--color-primary);
		outline-offset: -2px;
	}

	/* Show mobile header only on small screens */
	@media (max-width: 960px) {
		.mobile-header {
			display: block;
		}
	}
</style>
