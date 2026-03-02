import React, { createContext, useContext, useState, useEffect } from 'react';

type Theme = 'light' | 'dark';

interface ThemeContextType {
  theme: Theme;
  setTheme: (theme: Theme) => void;
  toggleTheme: () => void;
}

const ThemeContext = createContext<ThemeContextType | undefined>(undefined);

export const ThemeProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [theme, setThemeState] = useState<Theme>(() => {
    try {
      const stored = typeof localStorage !== 'undefined' ? localStorage.getItem('gaply_theme') : null;
      return stored === 'light' || stored === 'dark' ? stored : 'dark';
    } catch {
      return 'dark';
    }
  });

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    if (typeof document !== 'undefined') {
      if (theme === 'light') document.body.classList.add('light');
      else document.body.classList.remove('light');
    }
    try {
      if (typeof localStorage !== 'undefined') localStorage.setItem('gaply_theme', theme);
    } catch { /* ignore */ }
  }, [theme]);

  const setTheme = (t: Theme) => {
    if (typeof document !== 'undefined') document.body.classList.add('theme-transitioning');
    setThemeState(t);
    setTimeout(() => {
      if (typeof document !== 'undefined') document.body.classList.remove('theme-transitioning');
    }, 600);
  };
  const toggleTheme = () => {
    if (typeof document !== 'undefined') document.body.classList.add('theme-transitioning');
    setThemeState((prev) => (prev === 'dark' ? 'light' : 'dark'));
    setTimeout(() => {
      if (typeof document !== 'undefined') document.body.classList.remove('theme-transitioning');
    }, 600);
  };

  return (
    <ThemeContext.Provider value={{ theme, setTheme, toggleTheme }}>
      {children}
    </ThemeContext.Provider>
  );
};

export const useTheme = () => {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error('useTheme must be used within ThemeProvider');
  return ctx;
};
