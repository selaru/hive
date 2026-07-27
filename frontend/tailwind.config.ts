/** @type {import('tailwindcss').Config} */
export default {
  content: [
    './index.html',
    './src/**/*.{js,ts,jsx,tsx}',
  ],
  theme: {
    extend: {
      colors: {
        hive: {
          50:  '#fef9ec',
          100: '#fdf0cc',
          500: '#f59e0b',
          600: '#d97706',
          900: '#78350f',
        },
      },
    },
  },
  plugins: [],
}
