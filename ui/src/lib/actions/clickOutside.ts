import type { Action } from 'svelte/action';

/**
 * Svelte action that calls a handler when clicking outside the element
 */
export const clickOutside: Action<HTMLElement, (event: MouseEvent) => void> = (node, handler) => {
  const handleClick = (event: MouseEvent) => {
    if (node && !node.contains(event.target as Node) && !event.defaultPrevented) {
      handler?.(event);
    }
  };

  document.addEventListener('click', handleClick, true);

  return {
    destroy() {
      document.removeEventListener('click', handleClick, true);
    },
    update(newHandler) {
      handler = newHandler;
    }
  };
};
