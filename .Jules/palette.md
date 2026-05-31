## 2024-05-31 - Context-Aware ARIA Labels for Table Actions
**Learning:** Adding ARIA labels to icon-only buttons in data tables significantly improves accessibility, but static labels (e.g., "Delete") lack context for screen reader users when navigating tabular data.
**Action:** Always inject row-specific context into ARIA labels for action buttons within tables or lists, such as `aria-label="Delete user {row.username}"` or `aria-label="Download artifact for job trace {row.trace_id}"`, to provide clear intent for each action.
