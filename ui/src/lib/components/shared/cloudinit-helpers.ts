/**
 * Helpers used by `CloudInitEditor.svelte` to extract template variables and
 * apply a tiny client-side YAML syntax highlighter for the preview pane.
 */

export interface CloudInitSnippet {
  name: string;
  snippet: string;
}

export const COMMON_SNIPPETS: CloudInitSnippet[] = [
  { name: 'User', snippet: 'users:\n  - name: {{.Username}}\n    sudo: ALL=(ALL) NOPASSWD:ALL\n    ssh_authorized_keys:\n      - {{.SSHKey}}' },
  { name: 'Package', snippet: 'packages:\n  - package-name' },
  { name: 'Runcmd', snippet: 'runcmd:\n  - echo "Hello World"' },
  { name: 'Write Files', snippet: 'write_files:\n  - path: /etc/example.conf\n    content: |\n      example content' },
  { name: 'Hostname', snippet: 'hostname: {{.Hostname}}\nmanage_etc_hosts: true' },
];

export function extractVariables(content: string): string[] {
  const vars: string[] = [];
  const seen = new Set<string>();
  const regex = /\{\{\s*\.([A-Za-z][A-Za-z0-9_]*)\s*\}\}/g;
  let match;
  while ((match = regex.exec(content)) !== null) {
    if (!seen.has(match[1])) {
      seen.add(match[1]);
      vars.push(match[1]);
    }
  }
  return vars;
}

export function highlightYAML(content: string): string {
  if (!content) return '';

  return content
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    // Comments
    .replace(/(#.*$)/gm, '<span class="text-neutral-500">$1</span>')
    // Keys
    .replace(/^(\s*)([a-zA-Z_][a-zA-Z0-9_]*)(:)/gm, '$1<span class="text-sky-400">$2</span>$3')
    // String values (basic)
    .replace(/(:\s*)'(.*?)'/g, '$1<span class="text-emerald-400">\'$2\'</span>')
    // Numbers
    .replace(/(:\s*)(\d+)/g, '$1<span class="text-amber-400">$2</span>')
    // Template variables
    .replace(/(\{\{.*?\}\})/g, '<span class="text-pink-400">$1</span>');
}
