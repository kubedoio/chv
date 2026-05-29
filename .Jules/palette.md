## 2026-05-29 - Adding ARIA Labels to Icon-Only Buttons
**Learning:** Icon-only buttons frequently lack `aria-label` attributes, making them inaccessible to screen readers. In some cases, `title` attributes contain internal technical identifiers (e.g., `MODIFY_ENTITY`) instead of user-friendly text.
**Action:** Ensure all icon-only buttons include descriptive `aria-label` attributes. When updating, also check and replace technical `title` attributes with descriptive, user-friendly labels to improve tooltip usability.
