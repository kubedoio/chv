## 2024-05-17 - Missing ARIA Labels on Toggle Buttons
**Learning:** When implementing icon-only buttons for toggling visibility states (like Show/Hide Preview), a common pattern is to include text for the "Show" state but omit it (using only an icon) for the "Hide" state. This omission frequently leads to missing accessibility labels.
**Action:** Always ensure `aria-label` and `title` attributes are provided on icon-only toggle buttons to maintain accessibility across all states.
