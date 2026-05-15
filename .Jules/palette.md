## 2024-05-15 - Expandable Component Accessibility
**Learning:** When creating accordion/expandable components, it is not enough to just use `aria-expanded` on the trigger. Screen readers need a programmatic connection between the trigger and the content it controls.
**Action:** Ensure the trigger button has both `aria-expanded` and `aria-controls` pointing to the content ID. The expanded panel must contain a matching `id`, `role="region"`, and `aria-labelledby` linking back to the trigger.
