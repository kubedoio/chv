## 2024-05-22 - Adding robust aria controls for accordion components
**Learning:** Expanding/collapsing content (like accordions or detailed event items) requires explicit linkage between the trigger button and the content area so screen readers announce state correctly.
**Action:** Always ensure the trigger button has `aria-expanded` and `aria-controls`. The target expanded panel must contain a matching `id`, `role="region"`, and `aria-labelledby` referencing the trigger's `id`.
