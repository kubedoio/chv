<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		children: Snippet;
		active?: boolean;
		onEscape?: () => void;
		restoreFocus?: boolean;
	}

	let { 
		children, 
		active = true, 
		onEscape,
		restoreFocus = true 
	}: Props = $props();

	let containerRef = $state<HTMLDivElement | null>(null);
	let previouslyFocusedElement = $state<HTMLElement | null>(null);
	let isInitialized = $state(false);

	// Selectors for focusable elements
	const FOCUSABLE_SELECTORS = [
		'button:not([disabled]):not([aria-hidden="true"])',
		'a[href]:not([aria-hidden="true"])',
		'input:not([disabled]):not([type="hidden"]):not([aria-hidden="true"])',
		'select:not([disabled]):not([aria-hidden="true"])',
		'textarea:not([disabled]):not([aria-hidden="true"])',
		'[tabindex]:not([tabindex="-1"]):not([aria-hidden="true"])',
		'[contenteditable]:not([contenteditable="false"])'
	].join(', ');

	function getFocusableElements(): HTMLElement[] {
		if (!containerRef) return [];
		return Array.from(containerRef.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTORS))
			.filter(el => {
				// Check if element is actually visible
				const style = window.getComputedStyle(el);
				return style.display !== 'none' && style.visibility !== 'hidden';
			});
	}

	function handleKeyDown(event: KeyboardEvent) {
		if (!active) return;

		if (event.key === 'Escape' && onEscape) {
			event.preventDefault();
			onEscape();
			return;
		}

		if (event.key !== 'Tab') return;

		const focusableElements = getFocusableElements();
		if (focusableElements.length === 0) {
			event.preventDefault();
			return;
		}

		const firstElement = focusableElements[0];
		const lastElement = focusableElements[focusableElements.length - 1];

		if (event.shiftKey) {
			// Shift+Tab - move backwards
			if (document.activeElement === firstElement || !containerRef?.contains(document.activeElement)) {
				event.preventDefault();
				lastElement.focus();
			}
		} else {
			// Tab - move forwards
			if (document.activeElement === lastElement || !containerRef?.contains(document.activeElement)) {
				event.preventDefault();
				firstElement.focus();
			}
		}
	}

	// Initialize focus trap
	$effect(() => {
		if (active && containerRef && !isInitialized) {
			previouslyFocusedElement = document.activeElement as HTMLElement;
			
			// Focus the first focusable element
			const focusableElements = getFocusableElements();
			if (focusableElements.length > 0) {
				// Small delay to allow the DOM to settle
				setTimeout(() => {
					focusableElements[0].focus();
				}, 0);
			}
			isInitialized = true;
		}
	});

	// Cleanup - restore focus
	$effect(() => {
		return () => {
			if (restoreFocus && previouslyFocusedElement && document.contains(previouslyFocusedElement)) {
				previouslyFocusedElement.focus();
			}
		};
	});
</script>

<svelte:window onkeydown={handleKeyDown} />

<div bind:this={containerRef} class="focus-trap-container">
	{@render children()}
</div>

<style>
	.focus-trap-container {
		display: contents;
	}
</style>
