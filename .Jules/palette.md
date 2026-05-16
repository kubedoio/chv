## 2023-10-27 - Accordion Accessibility Pattern
**Learning:** Expanding list items require explicit linkage between the trigger button and the expanding content area for screen readers to properly associate them.
**Action:** Always add `aria-controls` to the trigger button, and give the expanding content matching `id`, `role="region"`, and `aria-labelledby`.
