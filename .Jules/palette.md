## 2024-05-02 - Icon-Only Button Accessibility Pattern
**Learning:** Found an icon-only "X" close button in the `ColumnVisibilityDropdown.svelte` component that lacked an ARIA label. This is a common pattern for modal/menu close buttons that impacts screen reader accessibility.
**Action:** When adding close buttons or other icon-only functional elements in the UI, always include an `aria-label` attribute (e.g., `aria-label="Close column menu"`) to provide context for assistive technologies.
