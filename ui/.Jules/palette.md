## 2025-05-27 - Icon-only buttons need aria-labels
**Learning:** Icon-only buttons without text content need `aria-label`s for accessibility, as `title` attributes alone aren't always sufficient or screen-reader friendly across all browsers.
**Action:** When updating or creating icon-only buttons (`class="btn-icon"`), always include both a `title` (for mouse hover) and an `aria-label` (for screen readers) with descriptive action text like "Edit user" or "Delete user".
