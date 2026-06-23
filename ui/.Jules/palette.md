## 2024-06-22 - ARIA Tab Components Svelte Pattern
**Learning:** When adding `role="tabpanel"` to structure a custom tab interface in SvelteKit, applying it directly to elements like `<main>` or `<form>` triggers a11y compiler warnings (`a11y_no_noninteractive_element_to_interactive_role`).
**Action:** Always wrap the tab content in a generic `<div>` with `role="tabpanel"`. To ensure this wrapper doesn't break existing CSS Grid or Flexbox parent-child layouts, apply `class="contents"` to the wrapper `<div>`.
