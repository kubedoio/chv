let isOpen = $state(false);

export function getIsOpen(): boolean {
	return isOpen;
}

export function openCommandPalette(): void {
	isOpen = true;
}
