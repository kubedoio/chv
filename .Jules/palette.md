
## 2026-05-28 - Improve Button Titles & A11y
**Learning:** Svelte component attributes like 'title' often retain internal keys (e.g., 'MODIFY_ENTITY') which degrade user experience. Icon-only buttons must have descriptive ARIA labels.
**Action:** Ensure all icon-only buttons include 'aria-label' and that tooltip/title text uses human-readable formatting instead of internal entity keys.
