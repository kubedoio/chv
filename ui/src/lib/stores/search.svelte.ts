import { browser } from '$app/environment';
import Fuse from 'fuse.js';
import type { FuseResultMatch } from 'fuse.js';
import type { VM, Image, Network, StoragePool } from '$lib/api/types';

// Types
export type SearchItemType = 'vm' | 'image' | 'network' | 'storage' | 'page';

export interface SearchItem {
  id: string;
  type: SearchItemType;
  name: string;
  description?: string;
  route?: string;
  meta?: Record<string, unknown>;
}

export interface SearchResult {
  item: SearchItem;
  score: number;
  matches?: readonly FuseResultMatch[];
}

// State
let searchIndex = $state<SearchItem[]>([]);
let recentSearches = $state<SearchItem[]>([]);
let fuse = $state<Fuse<SearchItem> | null>(null);
let isOpen = $state(false);
let searchQuery = $state('');
let debounceTimer = $state<number | null>(null);

const RECENT_SEARCHES_KEY = 'chv-recent-searches';
const MAX_RECENT_SEARCHES = 10;
const DEBOUNCE_MS = 300;

// Page definitions for search
const pages: SearchItem[] = [
  { id: 'dashboard', type: 'page', name: 'Dashboard', description: 'Overview and system status', route: '/' },
  { id: 'vms', type: 'page', name: 'Virtual Machines', description: 'Manage VMs', route: '/vms' },
  { id: 'images', type: 'page', name: 'Images', description: 'OS images and templates', route: '/images' },
  { id: 'networks', type: 'page', name: 'Networks', description: 'Network configuration', route: '/networks' },
  { id: 'storage', type: 'page', name: 'Storage', description: 'Storage pools', route: '/storage' },
  { id: 'events', type: 'page', name: 'Events', description: 'System events and logs', route: '/events' },
  { id: 'settings', type: 'page', name: 'Settings', description: 'System settings', route: '/settings' }
];

// Initialize fuse
function initFuse() {
  fuse = new Fuse(searchIndex, {
    keys: [
      { name: 'name', weight: 0.5 },
      { name: 'id', weight: 0.3 },
      { name: 'description', weight: 0.2 }
    ],
    threshold: 0.3,
    includeScore: true,
    includeMatches: true
  });
}

// Load recent searches from localStorage
export function loadRecentSearches() {
  if (!browser) return;
  
  try {
    const stored = localStorage.getItem(RECENT_SEARCHES_KEY);
    if (stored) {
      recentSearches = JSON.parse(stored);
    }
  } catch {
    recentSearches = [];
  }
}

// Save recent searches to localStorage
function saveRecentSearches() {
  if (!browser) return;
  
  try {
    localStorage.setItem(RECENT_SEARCHES_KEY, JSON.stringify(recentSearches.slice(0, MAX_RECENT_SEARCHES)));
  } catch {
    // Ignore storage errors
  }
}

// Add to recent searches
export function addToRecentSearches(item: SearchItem) {
  // Remove if already exists
  recentSearches = recentSearches.filter(r => r.id !== item.id || r.type !== item.type);
  // Add to front
  recentSearches = [item, ...recentSearches].slice(0, MAX_RECENT_SEARCHES);
  saveRecentSearches();
}

// Clear recent searches
export function clearRecentSearches() {
  recentSearches = [];
  saveRecentSearches();
}

// Build search index from data
export function buildSearchIndex(data: {
  vms?: VM[];
  images?: Image[];
  networks?: Network[];
  storagePools?: StoragePool[];
}) {
  const items: SearchItem[] = [...pages];
  
  // Add VMs
  if (data.vms) {
    for (const vm of data.vms) {
      items.push({
        id: vm.id,
        type: 'vm',
        name: vm.name,
        description: `State: ${vm.actual_state}${vm.ip_address ? ` | IP: ${vm.ip_address}` : ''}`,
        route: `/vms/${vm.id}`,
        meta: {
          state: vm.actual_state,
          ip: vm.ip_address,
          vcpu: vm.vcpu,
          memory: vm.memory_mb
        }
      });
    }
  }
  
  // Add Images
  if (data.images) {
    for (const img of data.images) {
      items.push({
        id: img.id,
        type: 'image',
        name: img.name,
        description: `${img.os_family} | ${img.architecture}`,
        route: '/images',
        meta: {
          osFamily: img.os_family,
          arch: img.architecture,
          format: img.format
        }
      });
    }
  }
  
  // Add Networks
  if (data.networks) {
    for (const net of data.networks) {
      items.push({
        id: net.id,
        type: 'network',
        name: net.name,
        description: `${net.mode}${net.bridge_name ? ` | ${net.bridge_name}` : ''}${net.cidr ? ` | ${net.cidr}` : ''}`,
        route: '/networks',
        meta: {
          mode: net.mode,
          bridge: net.bridge_name,
          cidr: net.cidr
        }
      });
    }
  }
  
  // Add Storage Pools
  if (data.storagePools) {
    for (const pool of data.storagePools) {
      items.push({
        id: pool.id,
        type: 'storage',
        name: pool.name,
        description: `${pool.pool_type}${pool.is_default ? ' | Default' : ''}`,
        route: '/storage',
        meta: {
          type: pool.pool_type,
          isDefault: pool.is_default
        }
      });
    }
  }
  
  searchIndex = items;
  initFuse();
}

// Search function with debounce
export function search(query: string): SearchResult[] {
  searchQuery = query;
  
  if (!fuse || !query.trim()) {
    return [];
  }
  
  const results = fuse.search(query);
  return results.map(r => ({
    item: r.item,
    score: r.score ?? 1,
    matches: r.matches
  }));
}

// Debounced search
export function searchDebounced(query: string, callback: (results: SearchResult[]) => void) {
  if (debounceTimer) {
    clearTimeout(debounceTimer);
  }
  
  debounceTimer = window.setTimeout(() => {
    callback(search(query));
  }, DEBOUNCE_MS);
}

// Get grouped results
export function getGroupedResults(results: SearchResult[]): Map<SearchItemType, SearchResult[]> {
  const grouped = new Map<SearchItemType, SearchResult[]>();
  
  for (const result of results) {
    const type = result.item.type;
    if (!grouped.has(type)) {
      grouped.set(type, []);
    }
    grouped.get(type)!.push(result);
  }
  
  return grouped;
}

// Type labels for display
export const typeLabels: Record<SearchItemType, string> = {
  vm: 'VMs',
  image: 'Images',
  network: 'Networks',
  storage: 'Storage',
  page: 'Pages'
};

// Type icons (using simple identifiers that components can map to icons)
export const typeIcons: Record<SearchItemType, string> = {
  vm: 'server',
  image: 'image',
  network: 'network',
  storage: 'hard-drive',
  page: 'file'
};

// Modal state
export function openSearch() {
  isOpen = true;
  // Load recent searches if not loaded
  if (recentSearches.length === 0) {
    loadRecentSearches();
  }
}

export function closeSearch() {
  isOpen = false;
  searchQuery = '';
  if (debounceTimer) {
    clearTimeout(debounceTimer);
    debounceTimer = null;
  }
}

export function toggleSearch() {
  if (isOpen) {
    closeSearch();
  } else {
    openSearch();
  }
}

// Getters for state
export function getIsOpen(): boolean {
  return isOpen;
}

export function getSearchQuery(): string {
  return searchQuery;
}

export function getRecentSearches(): SearchItem[] {
  return recentSearches;
}

export function setSearchQuery(query: string) {
  searchQuery = query;
}

// Escape HTML entities to prevent XSS
function escapeHtml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

// Highlight matches in text
export function highlightMatches(text: string, matches: readonly FuseResultMatch[] | undefined, key: string): string {
  if (!matches) return escapeHtml(text);
  
  const match = matches.find(m => m.key === key);
  if (!match) return escapeHtml(text);
  
  let result = '';
  let lastIndex = 0;
  
  // Sort indices by start position
  const indices = [...match.indices].sort((a, b) => a[0] - b[0]);
  
  for (const [start, end] of indices) {
    result += escapeHtml(text.slice(lastIndex, start));
    result += `<mark class="search-highlight">${escapeHtml(text.slice(start, end + 1))}</mark>`;
    lastIndex = end + 1;
  }
  
  result += escapeHtml(text.slice(lastIndex));
  return result;
}
