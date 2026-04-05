/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{vue,js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      fontFamily: {
        sans: ['Roboto', 'sans-serif'],
        mono: ['Roboto Mono', 'monospace'],
      },
      colors: {
        primary: '#0066CC',
        success: '#54B435',
        warning: '#F0AB00',
        error: '#E60000',
        chrome: '#F5F5F5',
        content: '#FFFFFF',
        border: '#D0D0D0',
        'text-primary': '#1A1A1A',
        'text-secondary': '#666666',
        hover: '#E8F4FC',
        selected: '#CCE5F9',
      },
      fontSize: {
        'xs': ['12px', { lineHeight: '16px' }],
        'sm': ['14px', { lineHeight: '20px' }],
        'base': ['14px', { lineHeight: '20px' }],
        'lg': ['16px', { lineHeight: '24px' }],
        'xl': ['20px', { lineHeight: '28px' }],
      },
      spacing: {
        '1': '4px',
        '2': '8px',
        '3': '12px',
        '4': '16px',
        '5': '24px',
        '6': '32px',
      }
    },
  },
  plugins: [],
}
