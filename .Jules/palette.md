
## 2023-10-27 - [Dynamic ARIA labels in tables]
**Learning:** [When dealing with table actions that toggle state, the aria label should dynamically reflect the action that will happen on click, rather than the current state]
**Action:** [Ensure toggle buttons in tables have dynamic aria-labels using ternary operators based on row state]

## 2024-05-18 - Missing ARIA labels in tables and consoles
**Learning:** Found older, internal tables (`TemplatesTable`) and console toolbars (`VmConsole`) utilizing icon-only buttons with just a `title` attribute for tooltips, missing the required `aria-label` for screen reader accessibility. Title alone is not reliably read by all assistive technologies.
**Action:** When adding or reviewing quick action buttons (like Copy, Download, Reconnect), always ensure an `aria-label` is present in addition to `title`.
