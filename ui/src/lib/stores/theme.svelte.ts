const STORAGE_KEY = 'chv-theme';

function getInitialTheme(): 'light' | 'dark' {
	if (typeof localStorage === 'undefined') return 'light';
	const stored = localStorage.getItem(STORAGE_KEY);
	if (stored === 'dark' || stored === 'light') return stored;
	if (typeof window !== 'undefined' && window.matchMedia('(prefers-color-scheme: dark)').matches)
		return 'dark';
	return 'light';
}

let current = $state<'light' | 'dark'>(getInitialTheme());

export const theme = {
	get value() {
		return current;
	},
	toggle() {
		current = current === 'light' ? 'dark' : 'light';
		if (typeof localStorage !== 'undefined') localStorage.setItem(STORAGE_KEY, current);
		if (typeof document !== 'undefined') document.documentElement.dataset.theme = current;
	},
	init() {
		if (typeof document !== 'undefined') document.documentElement.dataset.theme = current;
	}
};
