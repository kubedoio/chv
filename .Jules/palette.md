## 2026-05-10 - Accessible Accordion Components
**Learning:** When creating or fixing accordion/expandable components, the trigger button must have both `aria-expanded` and `aria-controls`. The expanded panel must contain a matching `id`, `role="region"`, and `aria-labelledby` linking back to the trigger.
**Action:** Ensure all expandable lists and accordions include this complete set of ARIA attributes to properly associate the trigger with the content it controls for screen reader users.
