/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      // Mirrors the --ease-* and --duration-* tokens in the stylesheet.
      transitionTimingFunction: {
        standard: 'cubic-bezier(0.25, 0.1, 0.25, 1.0)',
        decelerate: 'cubic-bezier(0.0, 0.0, 0.2, 1.0)',
        accelerate: 'cubic-bezier(0.4, 0.0, 1.0, 1.0)',
      },
      transitionDuration: {
        micro: '100ms',
        short: '150ms',
        medium: '200ms',
      },
    },
  },
  plugins: [],
};
