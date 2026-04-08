import { w as writable } from "./index.js";
function generateId() {
  return `${Date.now().toString(36)}-${Math.random().toString(36).substring(2, 9)}`;
}
function createToastStore() {
  const { subscribe, update } = writable({ toasts: [] });
  const timeouts = /* @__PURE__ */ new Map();
  function showToast(message, type, duration) {
    const id = generateId();
    const toast2 = { id, type, message, duration };
    update((state) => ({
      toasts: [...state.toasts, toast2]
    }));
    if (duration !== void 0 && duration > 0) {
      const timeout = setTimeout(() => {
        dismiss(id);
      }, duration);
      timeouts.set(id, timeout);
    }
  }
  function dismiss(id) {
    const timeout = timeouts.get(id);
    if (timeout) {
      clearTimeout(timeout);
      timeouts.delete(id);
    }
    update((state) => ({
      toasts: state.toasts.filter((t) => t.id !== id)
    }));
  }
  function success(message) {
    showToast(message, "success", 5e3);
  }
  function error(message) {
    showToast(message, "error");
  }
  function info(message) {
    showToast(message, "info", 5e3);
  }
  return {
    subscribe,
    showToast,
    success,
    error,
    info,
    dismiss
  };
}
const toast = createToastStore();
export {
  toast as t
};
