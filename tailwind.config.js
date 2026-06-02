/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        bg:      '#0f0f0f',
        surface: '#1a1a1a',
        border:  '#2a2a2a',
        accent:  '#5b8af0',
        text: {
          primary: '#f0f0f0',
          muted:   '#666666',
        },
      },
      borderRadius: { btn: '4px', card: '8px' },
    },
  },
  plugins: [],
};
