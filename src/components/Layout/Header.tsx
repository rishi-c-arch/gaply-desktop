import React from 'react';
import { Home, Menu } from 'lucide-react';

import './Header.css';

interface HeaderProps {
  onMenuClick?: () => void;
  showMenuButton?: boolean;
  theme?: 'light' | 'dark';
  onToggleTheme?: () => void;
  pageTitle?: string;
}

const Header: React.FC<HeaderProps> = ({ onMenuClick, showMenuButton = false, theme = 'dark', onToggleTheme, pageTitle = 'Overview' }) => {
  return (
    <header className="dashboard-header">
      <div className="dashboard-header__left">
        {showMenuButton && (
          <button
            type="button"
            onClick={onMenuClick}
            className="dashboard-header__menu-btn"
            aria-label="Open menu"
          >
            <Menu size={20} color="var(--dashboard-text-muted)" />
          </button>
        )}
        <Home size={20} color="var(--dashboard-text-muted)" className="dashboard-header__home-icon" />
        <h1 className="dashboard-header__title">{pageTitle}</h1>
      </div>

      <div className="dashboard-header__right">
        {onToggleTheme && (
          <button
            type="button"
            onClick={onToggleTheme}
            className="theme-toggle"
            aria-label={theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
          >
            {theme === 'dark' ? 'Light Mode' : 'Dark Mode'}
          </button>
        )}
      </div>
    </header>
  );
};

export default Header;
