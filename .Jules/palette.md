
## 2023-10-27 - [Dynamic ARIA labels in tables]
**Learning:** [When dealing with table actions that toggle state, the aria label should dynamically reflect the action that will happen on click, rather than the current state]
**Action:** [Ensure toggle buttons in tables have dynamic aria-labels using ternary operators based on row state]
## 2023-10-27 - [Dynamic ARIA labels for template row actions]
**Learning:** [When dealing with table row actions (e.g., clone, view) represented by icon-only buttons, static aria labels or none at all make it difficult for screen reader users to identify which item the action applies to.]
**Action:** [Ensure icon-only action buttons within table rows have dynamic aria-labels (e.g., `aria-label="Clone template {row.name}"`) so screen readers explicitly announce the target of the action.]
