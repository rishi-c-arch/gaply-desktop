import React from 'react';
import { Home, Mail, Bell, ChevronDown, Menu, Sun, Moon } from 'lucide-react';

interface HeaderProps {
  onMenuClick?: () => void;
  showMenuButton?: boolean;
  theme?: 'light' | 'dark';
  onToggleTheme?: () => void;
}

const Header: React.FC<HeaderProps> = ({ onMenuClick, showMenuButton = false, theme = 'dark', onToggleTheme }) => {
  return (
    <header
      style={{
        height: 56,
        background: 'var(--dashboard-header-bg)',
        borderBottom: '1px solid var(--dashboard-border)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        padding: '0 24px',
        position: 'sticky',
        top: 0,
        zIndex: 30,
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        {showMenuButton && (
          <button
            onClick={onMenuClick}
            style={{ background: 'transparent', border: 'none', cursor: 'pointer', padding: 8 }}
          >
            <Menu size={20} color="var(--dashboard-text-muted)" />
          </button>
        )}
        <Home size={20} color="var(--dashboard-text-muted)" />
        <h1 style={{ fontSize: 20, fontWeight: 600, color: 'var(--dashboard-text)', margin: 0 }}>
          Overview
        </h1>
      </div>

      <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
        {onToggleTheme && (
          <button
            onClick={onToggleTheme}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 6,
              background: 'var(--dashboard-sidebar-active-bg)',
              border: '1px solid var(--dashboard-border)',
              borderRadius: 8,
              padding: '6px 12px',
              color: 'var(--dashboard-text)',
              fontSize: 13,
              fontWeight: 500,
              cursor: 'pointer',
            }}
          >
            {theme === 'dark' ? <Sun size={16} /> : <Moon size={16} />}
            {theme === 'dark' ? 'Light Mode' : 'Dark Mode'}
          </button>
        )}
        <button style={{ background: 'transparent', border: 'none', cursor: 'pointer', padding: 8 }}>
          <Mail size={20} color="var(--dashboard-text-muted)" />
        </button>
        <button style={{ background: 'transparent', border: 'none', cursor: 'pointer', padding: 8, position: 'relative' }}>
          <Bell size={20} color="var(--dashboard-text-muted)" />
          <span
            style={{
              position: 'absolute',
              top: 6,
              right: 6,
              width: 8,
              height: 8,
              borderRadius: '50%',
              background: '#EF4444',
            }}
          />
        </button>
        <button
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 8,
            background: 'transparent',
            border: 'none',
            cursor: 'pointer',
            padding: '4px 8px',
          }}
        >
          <div
            style={{
              width: 36,
              height: 36,
              borderRadius: '50%',
              background: 'linear-gradient(135deg, #3B82F6 0%, #8B5CF6 100%)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              color: 'white',
              fontSize: 14,
              fontWeight: 600,
            }}
          >
            U
          </div>
          <ChevronDown size={16} color="var(--dashboard-text-muted)" />
        </button>
      </div>
    </header>
  );
};

export default Header;
