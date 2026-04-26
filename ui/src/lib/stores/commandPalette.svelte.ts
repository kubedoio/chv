let isOpen = $state(false);

export function getIsOpen(): boolean {
	return isOpen;
}

export function openCommandPalette(): void {
	isOpen = true;
}

export function closeCommandPalette(): void {
	isOpen = false;
}

export function toggleCommandPalette(): void {
	isOpen = !isOpen;
}
