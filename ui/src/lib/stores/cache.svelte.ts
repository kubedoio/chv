export class ReactiveCache<T> {
	private ttlMs: number;
	private data = $state<Map<string, { value: T; timestamp: number }>>(new Map());

	constructor(ttlMs: number = 30000) {
		this.ttlMs = ttlMs;
	}

	get(key: string): T | undefined {
		const cached = this.data.get(key);
		if (!cached) return undefined;

		if (Date.now() - cached.timestamp > this.ttlMs) {
			// Expired, but we'll return it and let the caller decide if they want to refetch in background
			return cached.value;
		}
		return cached.value;
	}

	set(key: string, value: T) {
		const newMap = new Map(this.data);
		newMap.set(key, { value, timestamp: Date.now() });
		this.data = newMap;
	}

	isValid(key: string): boolean {
		const cached = this.data.get(key);
		if (!cached) return false;
		return Date.now() - cached.timestamp <= this.ttlMs;
	}

	clear() {
		this.data = new Map();
	}
}

// Global instances for our heavily hit routes
export const nodeCache = new ReactiveCache<any>(30000);
export const vmCache = new ReactiveCache<any>(30000);
