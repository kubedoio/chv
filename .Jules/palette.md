
## 2023-10-27 - [Dynamic ARIA labels in tables]
**Learning:** [When dealing with table actions that toggle state, the aria label should dynamically reflect the action that will happen on click, rather than the current state]
**Action:** [Ensure toggle buttons in tables have dynamic aria-labels using ternary operators based on row state]
## 2024-06-13 - Dynamic ARIA Labels for Icon-Only Buttons in Tables
**Learning:** When using icon-only action buttons within table rows (like in ImagesTable or TemplatesTable), static labels (e.g. 'Clone template') are insufficient for screen readers as they lack context about *which* row is being acted upon.
**Action:** Use dynamic `aria-label` attributes that interpolate the row identifier (e.g., `aria-label="Clone template {row.name}"`) so the intent is fully accessible without visual context.

## 2026-06-11 - [ARIA labels on Icon Buttons]
**Learning:** [Many icon-only buttons in complex Svelte components lack `aria-label` or `title` attributes, making them inaccessible to screen readers and difficult for users to infer their action without context. Adding `title` provides a tooltip, while `aria-label` informs screen readers.]
**Action:** [Consistently review newly added or existing icon-only buttons to ensure they have descriptive `aria-label` and `title` attributes.]

## 2024-05-17 - ARIA pressed state for toggle buttons
**Learning:** Custom UI toggle buttons that visually indicate active/inactive state using CSS classes (like `dashboard-panel-toggle--active`) often miss the programmatic equivalent for screen readers.
**Action:** When creating or maintaining custom toggle buttons, always add an `aria-pressed` attribute bound to the same boolean state as the visual class indicator.
