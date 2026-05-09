## 2024-05-18 - Accordion Expandables Accessibility
**Learning:** Found an accordion structure (`EventList.svelte`) that visually expands but lacked the standard ARIA attributes (`aria-controls`, `role="region"`, `aria-labelledby`, and `id` links). By adding these, screen readers can now understand that the button toggles a specific region below it.
**Action:** When implementing expand/collapse lists or accordions, ensure both the trigger (button) and the panel (div) use proper ARIA attributes to cross-link one another.
