/** @type {import('tailwindcss').Config} */
module.exports = {
  darkMode: 'class',
  content: [
    "./src/**/*.{js,jsx,ts,tsx}",
    "./public/index.html"
  ],
  theme: {
    extend: {
      colors: {
        'on-secondary-fixed': 'rgb(var(--cm-on-secondary-fixed) / <alpha-value>)',
        'on-primary-fixed-variant': 'rgb(var(--cm-on-primary-fixed-variant) / <alpha-value>)',
        'tertiary': 'rgb(var(--cm-tertiary) / <alpha-value>)',
        'outline-variant': 'rgb(var(--cm-outline-variant) / <alpha-value>)',
        'inverse-on-surface': 'rgb(var(--cm-inverse-on-surface) / <alpha-value>)',
        'surface-container-high': 'rgb(var(--cm-surface-container-high) / <alpha-value>)',
        'primary-fixed': 'rgb(var(--cm-primary-fixed) / <alpha-value>)',
        'on-background': 'rgb(var(--cm-on-background) / <alpha-value>)',
        'on-secondary': 'rgb(var(--cm-on-secondary) / <alpha-value>)',
        'on-primary': 'rgb(var(--cm-on-primary) / <alpha-value>)',
        'surface-variant': 'rgb(var(--cm-surface-variant) / <alpha-value>)',
        'tertiary-container': 'rgb(var(--cm-tertiary-container) / <alpha-value>)',
        'on-primary-fixed': 'rgb(var(--cm-on-primary-fixed) / <alpha-value>)',
        'on-tertiary-fixed': 'rgb(var(--cm-on-tertiary-fixed) / <alpha-value>)',
        'surface-container': 'rgb(var(--cm-surface-container) / <alpha-value>)',
        'on-primary-container': 'rgb(var(--cm-on-primary-container) / <alpha-value>)',
        'tertiary-fixed': 'rgb(var(--cm-tertiary-fixed) / <alpha-value>)',
        'primary-fixed-dim': 'rgb(var(--cm-primary-fixed-dim) / <alpha-value>)',
        'on-tertiary-fixed-variant': 'rgb(var(--cm-on-tertiary-fixed-variant) / <alpha-value>)',
        'on-secondary-fixed-variant': 'rgb(var(--cm-on-secondary-fixed-variant) / <alpha-value>)',
        'inverse-surface': 'rgb(var(--cm-inverse-surface) / <alpha-value>)',
        'secondary-fixed': 'rgb(var(--cm-secondary-fixed) / <alpha-value>)',
        'secondary-fixed-dim': 'rgb(var(--cm-secondary-fixed-dim) / <alpha-value>)',
        'surface-container-low': 'rgb(var(--cm-surface-container-low) / <alpha-value>)',
        'surface-container-lowest': 'rgb(var(--cm-surface-container-lowest) / <alpha-value>)',
        'surface-dim': 'rgb(var(--cm-surface-dim) / <alpha-value>)',
        'on-tertiary-container': 'rgb(var(--cm-on-tertiary-container) / <alpha-value>)',
        'on-surface-variant': 'rgb(var(--cm-on-surface-variant) / <alpha-value>)',
        'primary-container': 'rgb(var(--cm-primary-container) / <alpha-value>)',
        'on-error-container': 'rgb(var(--cm-on-error-container) / <alpha-value>)',
        'error-container': 'rgb(var(--cm-error-container) / <alpha-value>)',
        'secondary': 'rgb(var(--cm-secondary) / <alpha-value>)',
        'surface-container-highest': 'rgb(var(--cm-surface-container-highest) / <alpha-value>)',
        'error': 'rgb(var(--cm-error) / <alpha-value>)',
        'surface-bright': 'rgb(var(--cm-surface-bright) / <alpha-value>)',
        'secondary-container': 'rgb(var(--cm-secondary-container) / <alpha-value>)',
        'on-error': 'rgb(var(--cm-on-error) / <alpha-value>)',
        'background': 'rgb(var(--cm-background) / <alpha-value>)',
        'on-surface': 'rgb(var(--cm-on-surface) / <alpha-value>)',
        'on-accent': 'rgb(var(--cm-on-accent) / <alpha-value>)',
        'outline': 'rgb(var(--cm-outline) / <alpha-value>)',
        'on-tertiary': 'rgb(var(--cm-on-tertiary) / <alpha-value>)',
        'inverse-primary': 'rgb(var(--cm-inverse-primary) / <alpha-value>)',
        'tertiary-fixed-dim': 'rgb(var(--cm-tertiary-fixed-dim) / <alpha-value>)',
        'surface-tint': 'rgb(var(--cm-surface-tint) / <alpha-value>)',
        'on-secondary-container': 'rgb(var(--cm-on-secondary-container) / <alpha-value>)',
        /* Non-colliding aliases: the project's own palette claims the bare
           'surface' (#FFFFFF) and 'primary' (blue scale) keys, so the Archive
           screen uses these instead. */
        'paper': 'rgb(var(--cm-surface) / <alpha-value>)',
        'brand': 'rgb(var(--cm-primary) / <alpha-value>)',
        dashboard: {
          bg: '#0D1117',
          card: '#161B22',
          border: '#30363D',
          accent: '#3B82F6',
          text: '#FFFFFF',
          muted: '#8B949E',
          success: '#22C55E',
          warning: '#6B7280',
          danger: '#EF4444',
        },
        primary: {
          DEFAULT: '#00A3FF',
          50: '#E6F7FF',
          100: '#CCEFFF',
          200: '#99DDFF',
          300: '#66CCFF',
          400: '#33BBFF',
          500: '#00A3FF',
          600: '#008FD6',
          700: '#006FA8',
          800: '#004F7A',
          900: '#002F4C'
        },
        neutral: {
          50: '#F8FAFC',
          100: '#F1F5F9',
          200: '#E2E8F0',
          300: '#CBD5E1',
          400: '#94A3B8',
          500: '#64748B',
          600: '#475569',
          700: '#334155',
          800: '#1E293B',
          900: '#0F1724'
        },
        surface: '#FFFFFF',
        muted: '#F8FAFC'
      },
      fontFamily: {
        sans: ['Inter', 'ui-sans-serif', 'system-ui', '-apple-system', 'BlinkMacSystemFont', 'Segoe UI', 'Roboto', 'Helvetica Neue', 'Arial', 'sans-serif'],
        mono: ['JetBrains Mono', 'ui-monospace', 'SFMono-Regular', 'Menlo', 'Monaco', 'Consolas', 'Liberation Mono', 'Courier New', 'monospace'],
        headline: ['"Libre Caslon Text"', 'Georgia', 'serif'],
        display: ['"Libre Caslon Text"', 'Georgia', 'serif'],
        body: ['var(--cm-font-body)', 'Georgia', 'serif'],
        label: ['var(--cm-font-label)', 'sans-serif']
      },
      fontSize: {
        'xs': ['0.75rem', { lineHeight: '1rem' }],
        'sm': ['0.875rem', { lineHeight: '1.25rem' }],
        'base': ['1rem', { lineHeight: '1.5rem' }],
        'lg': ['1.125rem', { lineHeight: '1.75rem' }],
        'xl': ['1.25rem', { lineHeight: '1.75rem' }],
        '2xl': ['1.5rem', { lineHeight: '2rem' }],
        '3xl': ['1.875rem', { lineHeight: '2.25rem' }],
        '4xl': ['2.25rem', { lineHeight: '2.5rem' }],
        '5xl': ['3rem', { lineHeight: '1' }],
        '6xl': ['3.75rem', { lineHeight: '1' }],
        '7xl': ['4.5rem', { lineHeight: '1' }],
        '8xl': ['6rem', { lineHeight: '1' }],
        '9xl': ['8rem', { lineHeight: '1' }]
      },
      spacing: {
        '18': '4.5rem',
        '22': '5.5rem',
        '26': '6.5rem',
        '30': '7.5rem'
      },
      borderRadius: {
        'DEFAULT': '0.125rem',
        'sm': '6px',
        'md': '12px',
        'lg': '0.25rem',
        'xl': '0.5rem',
        '2xl': '0.75rem',
        'full': '9999px'
      },
      boxShadow: {
        'soft': '0 6px 18px rgba(9,30,66,0.08)',
        'pop': '0 12px 30px rgba(9,30,66,0.12)',
        'glow': '0 0 20px rgba(0,163,255,0.3)',
        'card': '0 4px 16px rgba(0,0,0,0.1)'
      },
      animation: {
        'fade-in': 'fadeIn 0.5s ease-in-out',
        'slide-up': 'slideUp 0.5s ease-out',
        'float': 'float 3s ease-in-out infinite',
        'glow': 'glow 2s ease-in-out infinite alternate'
      },
      keyframes: {
        fadeIn: {
          '0%': { opacity: '0' },
          '100%': { opacity: '1' }
        },
        slideUp: {
          '0%': { transform: 'translateY(20px)', opacity: '0' },
          '100%': { transform: 'translateY(0)', opacity: '1' }
        },
        float: {
          '0%, 100%': { transform: 'translateY(0px)' },
          '50%': { transform: 'translateY(-10px)' }
        },
        glow: {
          '0%': { boxShadow: '0 0 20px rgba(0,163,255,0.3)' },
          '100%': { boxShadow: '0 0 30px rgba(0,163,255,0.5)' }
        }
      }
    }
  },
  plugins: []
}
