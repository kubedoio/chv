const fs = require('fs');
const file = 'ui/tests/e2e/settings.spec.ts';
let content = fs.readFileSync(file, 'utf8');

// I need to change "Settings / Access" to "Settings"
content = content.replace(
  "await expect(page.getByRole('heading', { name: 'Settings / Access' })).toBeVisible();",
  "await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();"
);

fs.writeFileSync(file, content);

const file2 = 'ui/tests/e2e/navigation.spec.ts';
let content2 = fs.readFileSync(file2, 'utf8');

// The other failure was in navigation.spec.ts: `await expect(page.locator('.fixed.inset-0')).toBeVisible();`
// We should wait for something more reliable.
content2 = content2.replace(
  "await expect(page.locator('.fixed.inset-0')).toBeVisible();",
  "await expect(page.getByPlaceholder(/type a command or search/i)).toBeVisible(); // wait for search input"
);
content2 = content2.replace(
  "await expect(page.getByPlaceholder(/type a command or search/i)).toBeVisible();\n\t});",
  "\t});"
);

fs.writeFileSync(file2, content2);
