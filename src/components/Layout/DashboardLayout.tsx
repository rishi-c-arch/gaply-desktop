import React, { useState, useEffect } from 'react';
import Sidebar from './Sidebar';
import Header from './Header';
import { useTheme } from '../../contexts/ThemeContext';

interface DashboardLayoutProps {
  children: React.ReactNode;
  pageTitle?: string;
}

const MOBILE_BREAKPOINT = 768;

const DashboardLayout: React.FC<DashboardLayoutProps> = ({ children, pageTitle }) => {
  const { theme, toggleTheme } = useTheme();
  const [collapsed, setCollapsed] = useState(false);
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const [isMobile, setIsMobile] = useState(typeof window !== 'undefined' && window.innerWidth < MOBILE_BREAKPOINT);

  useEffect(() => {
    const handler = () => {
      const mobile = window.innerWidth < MOBILE_BREAKPOINT;
      setIsMobile(mobile);
      if (!mobile) setMobileMenuOpen(false);
    };
    window.addEventListener('resize', handler);
    return () => window.removeEventListener('resize', handler);
  }, []);

  const sidebarWidth = isMobile ? 0 : (collapsed ? 64 : 240);
  const showSidebarOverlay = isMobile && mobileMenuOpen;

  return (
    <div style={{ display: 'flex', minHeight: '100vh', background: 'var(--dashboard-bg)' }}>
      {showSidebarOverlay && (
        <div
          onClick={() => setMobileMenuOpen(false)}
          style={{
            position: 'fixed',
            inset: 0,
            background: 'rgba(0,0,0,0.5)',
            zIndex: 35,
          }}
        />
      )}
      <Sidebar
        collapsed={collapsed}
        onCollapsedChange={setCollapsed}
        mobileOpen={showSidebarOverlay}
        onMobileClose={() => setMobileMenuOpen(false)}
        isMobile={isMobile}
      />
      <div style={{ flex: 1, marginLeft: sidebarWidth, transition: 'margin-left 150ms ease', display: 'flex', flexDirection: 'column' }}>
        <Header
          onMenuClick={() => setMobileMenuOpen(true)}
          showMenuButton={isMobile}
          theme={theme}
          onToggleTheme={toggleTheme}
          pageTitle={pageTitle}
        />
        <main style={{ flex: 1, padding: 'clamp(12px, 3vw, 24px)', overflow: 'auto', minWidth: 0 }}>
          {children}
        </main>
      </div>
    </div>
  );
};

export default DashboardLayout;
