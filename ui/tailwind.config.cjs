/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ['./src/**/*.{html,js,svelte,ts}'],
  theme: {
    extend: {
      fontFamily: {
        sans: ['Roboto', 'system-ui', 'sans-serif'],
        mono: ['Roboto Mono', 'ui-monospace', 'SFMono-Regular', 'monospace']
      },
      colors: {
        chrome: 'var(--bg-surface-muted, #F5F5F5)',
        line: 'var(--border-subtle, #D0D0D0)',
        ink: 'var(--shell-text, #1A1A1A)',
        muted: 'var(--shell-text-muted, #666666)',
        light: 'var(--shell-text-secondary, #999999)',
        primary: 'var(--color-primary, #8f5a2a)',
        success: 'var(--color-success, #3f6b45)',
        warning: 'var(--color-warning, #9a6a1f)',
        danger: 'var(--color-danger, #9b4338)',
        hover: 'var(--shell-accent-soft, #E8F4FC)',
        selected: 'var(--color-primary-light, #CCE5F9)'
      }
    }
  },
  plugins: []
};

