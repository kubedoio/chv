
## 2023-10-27 - [Dynamic ARIA labels in tables]
**Learning:** [When dealing with table actions that toggle state, the aria label should dynamically reflect the action that will happen on click, rather than the current state]
**Action:** [Ensure toggle buttons in tables have dynamic aria-labels using ternary operators based on row state]
## 2024-06-13 - Dynamic ARIA Labels for Icon-Only Buttons in Tables
**Learning:** When using icon-only action buttons within table rows (like in ImagesTable or TemplatesTable), static labels (e.g. 'Clone template') are insufficient for screen readers as they lack context about *which* row is being acted upon.
**Action:** Use dynamic `aria-label` attributes that interpolate the row identifier (e.g., `aria-label="Clone template {row.name}"`) so the intent is fully accessible without visual context.
