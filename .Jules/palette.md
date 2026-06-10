
## 2023-10-27 - [Dynamic ARIA labels in tables]
**Learning:** [When dealing with table actions that toggle state, the aria label should dynamically reflect the action that will happen on click, rather than the current state]
**Action:** [Ensure toggle buttons in tables have dynamic aria-labels using ternary operators based on row state]

## 2026-06-10 - [Contextual ARIA labels for destructive actions]
**Learning:** [When deleting a specific item from a list (like an image table row), generic aria-labels like 'Purge Image' provide insufficient context for screen reader users.]
**Action:** [Always append the specific item identifier (e.g. `{row.name}`) to the aria-label of destructive actions in list views to prevent accidental deletion and provide clear context.]
