import { browser } from '$app/environment';
import { goto } from '$app/navigation';

// Types
export interface Shortcut {
  id: string;
  key: string;
  modifiers?: ('ctrl' | 'meta' | 'alt' | 'shift')[];
  context?: 'global' | 'vms' | 'vm-detail' | 'navigation' | string;
  description: string;
  handler: (event: KeyboardEvent) => void | Promise<void>;
  preventDefault?: boolean;
  allowInInput?: boolean;
}

export interface ShortcutGroup {
  name: string;
  shortcuts: Shortcut[];
}

// State
let registeredShortcuts = $state<Shortcut[]>([]);
let activeContext = $state<string>('global');
let helpOpen = $state(false);
let shortcutSequence = $state<string[]>([]);
let sequenceTimeout = $state<number | null>(null);

const SEQUENCE_TIMEOUT_MS = 1000;

// Platform detection
const isMac = browser ? navigator.platform.toUpperCase().includes('MAC') : false;

export function getModifierKey(): string {
  return isMac ? '⌘' : 'Ctrl';
}

export function formatShortcut(shortcut: Shortcut): string {
  const parts: string[] = [];
  
  if (shortcut.modifiers) {
    if (shortcut.modifiers.includes('ctrl')) parts.push(isMac ? '⌃' : 'Ctrl');
    if (shortcut.modifiers.includes('meta')) parts.push(isMac ? '⌘' : 'Win');
    if (shortcut.modifiers.includes('alt')) parts.push(isMac ? '⌥' : 'Alt');
    if (shortcut.modifiers.includes('shift')) parts.push(isMac ? '⇧' : 'Shift');
  }
  
  parts.push(shortcut.key.toUpperCase());
  
  return parts.join(' + ');
}

// Check if an element is an input field
function isInputElement(element: Element | null): boolean {
  if (!element) return false;
  const tagName = element.tagName.toLowerCase();
  return tagName === 'input' || 
         tagName === 'textarea' || 
         tagName === 'select' ||
         element.getAttribute('contenteditable') === 'true';
}

// Check if modifiers match
function modifiersMatch(event: KeyboardEvent, expected: ('ctrl' | 'meta' | 'alt' | 'shift')[] | undefined): boolean {
  if (!expected || expected.length === 0) {
    return !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey;
  }
  
  const hasCtrl = expected.includes('ctrl');
  const hasMeta = expected.includes('meta');
  const hasAlt = expected.includes('alt');
  const hasShift = expected.includes('shift');
  
  return (
    event.ctrlKey === hasCtrl &&
    event.metaKey === hasMeta &&
    event.altKey === hasAlt &&
    event.shiftKey === hasShift
  );
}

// Handle keyboard event
function handleKeyDown(event: KeyboardEvent) {
  // Handle '?' for help (with shift, so it's Shift+?)
  if (event.key === '?' && !isInputElement(document.activeElement)) {
    event.preventDefault();
    helpOpen = true;
    return;
  }
  
  // Handle Escape to close help
  if (event.key === 'Escape' && helpOpen) {
    event.preventDefault();
    helpOpen = false;
    return;
  }
  
  const target = event.target as Element;
  const inInput = isInputElement(target);
  
  // Check for sequence shortcuts (like 'g' then 'd')
  if (!inInput && shortcutSequence.length > 0) {
    const currentSequence = [...shortcutSequence, event.key.toLowerCase()];
    
    // Find shortcuts that match this sequence
    const sequenceShortcut = registeredShortcuts.find(s => {
      if (s.modifiers && s.modifiers.length > 0) return false;
      const shortcutKey = s.key.toLowerCase();
      const expectedSequence = shortcutKey.split('');
      return (
        currentSequence.length === expectedSequence.length &&
        currentSequence.every((k, i) => k === expectedSequence[i])
      );
    });
    
    if (sequenceShortcut) {
      event.preventDefault();
      shortcutSequence = [];
      if (sequenceTimeout) {
        clearTimeout(sequenceTimeout);
        sequenceTimeout = null;
      }
      sequenceShortcut.handler(event);
      return;
    }
    
    // Check if we're starting a valid sequence
    const startsSequence = registeredShortcuts.some(s => {
      if (s.modifiers && s.modifiers.length > 0) return false;
      const shortcutKey = s.key.toLowerCase();
      return shortcutKey.length > 1 && shortcutKey.startsWith(currentSequence.join(''));
    });
    
    if (startsSequence) {
      event.preventDefault();
      shortcutSequence = currentSequence;
      if (sequenceTimeout) clearTimeout(sequenceTimeout);
      sequenceTimeout = window.setTimeout(() => {
        shortcutSequence = [];
      }, SEQUENCE_TIMEOUT_MS);
      return;
    }
    
    // Invalid sequence, reset
    shortcutSequence = [];
    if (sequenceTimeout) {
      clearTimeout(sequenceTimeout);
      sequenceTimeout = null;
    }
  }
  
  // Find matching shortcut
  const shortcut = registeredShortcuts.find(s => {
    // Check context match
    if (s.context && s.context !== 'global' && s.context !== activeContext) {
      return false;
    }
    
    // Check if allowed in input
    if (inInput && !s.allowInInput) {
      return false;
    }
    
    // Check modifiers
    if (!modifiersMatch(event, s.modifiers)) {
      return false;
    }
    
    // Check key
    const eventKey = event.key.toLowerCase();
    const shortcutKey = s.key.toLowerCase();
    
    // Handle multi-key sequences (like 'gd', 'gv')
    if (shortcutKey.length > 1) {
      // Start sequence
      if (eventKey === shortcutKey[0]) {
        shortcutSequence = [eventKey];
        if (sequenceTimeout) clearTimeout(sequenceTimeout);
        sequenceTimeout = window.setTimeout(() => {
          shortcutSequence = [];
        }, SEQUENCE_TIMEOUT_MS);
        return true;
      }
      return false;
    }
    
    return eventKey === shortcutKey;
  });
  
  if (shortcut) {
    // For multi-key shortcuts, we've already started the sequence above
    if (shortcut.key.length > 1) {
      event.preventDefault();
      return;
    }
    
    if (shortcut.preventDefault !== false) {
      event.preventDefault();
    }
    shortcut.handler(event);
  }
}

// Initialize keyboard shortcuts
export function initKeyboardShortcuts() {
  if (!browser) return;
  
  document.addEventListener('keydown', handleKeyDown);
  
  return () => {
    document.removeEventListener('keydown', handleKeyDown);
  };
}

// Register shortcuts
export function registerShortcuts(shortcuts: Shortcut[]) {
  registeredShortcuts = [...registeredShortcuts, ...shortcuts];
  
  return () => {
    registeredShortcuts = registeredShortcuts.filter(
      s => !shortcuts.some(ns => ns.id === s.id)
    );
  };
}

// Set active context
export function setActiveContext(context: string) {
  activeContext = context;
}

// Toggle help modal
export function toggleHelp(open?: boolean) {
  helpOpen = open ?? !helpOpen;
}

// Get help state
export function isHelpOpen(): boolean {
  return helpOpen;
}

// Get all shortcuts grouped by context
export function getShortcutsByContext(): ShortcutGroup[] {
  const groups = new Map<string, Shortcut[]>();
  
  for (const shortcut of registeredShortcuts) {
    const context = shortcut.context || 'global';
    if (!groups.has(context)) {
      groups.set(context, []);
    }
    groups.get(context)!.push(shortcut);
  }
  
  const contextOrder = ['global', 'navigation', 'vms', 'vm-detail'];
  const sortedGroups: ShortcutGroup[] = [];
  
  for (const context of contextOrder) {
    if (groups.has(context)) {
      sortedGroups.push({
        name: context,
        shortcuts: groups.get(context)!
      });
      groups.delete(context);
    }
  }
  
  // Add any remaining contexts
  for (const [name, shortcuts] of groups) {
    sortedGroups.push({ name, shortcuts });
  }
  
  return sortedGroups;
}

// Built-in global shortcuts
export function createGlobalShortcuts(
  onSearchOpen: () => void,
  onQuickActionsOpen: () => void,
  onCommandPaletteOpen: () => void
): Shortcut[] {
  return [
    {
      id: 'command-palette',
      key: 'k',
      modifiers: [isMac ? 'meta' : 'ctrl'],
      context: 'global',
      description: 'Open command palette',
      handler: onCommandPaletteOpen,
      preventDefault: true
    },
    {
      id: 'quick-actions',
      key: 'p',
      modifiers: [isMac ? 'meta' : 'ctrl', 'shift'],
      context: 'global',
      description: 'Open quick actions',
      handler: onQuickActionsOpen,
      preventDefault: true
    },
	    {
	      id: 'go-dashboard',
	      key: 'gd',
	      context: 'global',
	      description: 'Go to Overview',
	      handler: () => goto('/')
	    },
    {
      id: 'go-vms',
      key: 'gv',
      context: 'global',
      description: 'Go to VMs',
      handler: () => goto('/vms')
    },
    {
      id: 'go-images',
      key: 'gi',
      context: 'global',
      description: 'Go to Images',
      handler: () => goto('/images')
    },
	    {
	      id: 'go-volumes',
	      key: 'gs',
	      context: 'global',
	      description: 'Go to Volumes',
	      handler: () => goto('/volumes')
	    },
    {
      id: 'go-networks',
      key: 'gn',
      context: 'global',
      description: 'Go to Networks',
      handler: () => goto('/networks')
    }
  ];
}
