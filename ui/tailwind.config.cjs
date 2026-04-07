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
        chrome: '#F5F5F5',
        line: '#D0D0D0',
        ink: '#1A1A1A',
        muted: '#666666',
        primary: '#0066CC',
        success: '#54B435',
        warning: '#F0AB00',
        danger: '#E60000',
        hover: '#E8F4FC',
        selected: '#CCE5F9'
      }
    }
  },
  plugins: []
};

