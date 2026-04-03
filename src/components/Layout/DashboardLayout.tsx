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

  const sidebarWidth = isMobile ? 0 : (collapsed ? 64 : 260);
  const showSidebarOverlay = isMobile && mobileMenuOpen;

  return (
    <div
      className="dashboard-premium-wrapper dashboard-hud"
      style={{
        display: 'flex',
        minHeight: '100vh',
        background: 'var(--dashboard-bg)',
        fontFamily: "'Space Grotesk', sans-serif",
      }}
    >
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
      <div
        style={{
          flex: 1,
          marginLeft: sidebarWidth,
          transition: 'margin-left 150ms ease',
          display: 'flex',
          flexDirection: 'column',
          minWidth: 0,
        }}
      >
        <Header
          onMenuClick={() => setMobileMenuOpen(true)}
          showMenuButton={isMobile}
          theme={theme}
          onToggleTheme={toggleTheme}
          pageTitle={pageTitle}
        />
        <main style={{ flex: 1, padding: 'clamp(12px, 3vw, 24px)', overflow: 'auto', minWidth: 0, paddingBottom: 56 }}>
          {children}
        </main>
        {/* Footer HUD */}
        <footer
          style={{
            position: 'fixed',
            bottom: 0,
            left: sidebarWidth,
            right: 0,
            height: 32,
            borderTop: '1px solid var(--dashboard-border)',
            background: 'var(--dashboard-bg)',
            backdropFilter: 'blur(12px)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            paddingLeft: 24,
            paddingRight: 24,
            zIndex: 40,
            fontSize: 9,
            fontFamily: 'ui-monospace, monospace',
            color: 'var(--dashboard-text-muted)',
            letterSpacing: '0.05em',
            textTransform: 'uppercase',
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: 24 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <span
                style={{
                  width: 6,
                  height: 6,
                  borderRadius: '50%',
                  background: 'var(--dashboard-accent-emerald)',
                  animation: 'pulse 2s infinite',
                }}
              />
              <span>API_GATEWAY_V1: OPERATIONAL</span>
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <span className="material-symbols-outlined" style={{ fontSize: 12 }}>lock_open</span>
              <span>SECURE SESSION</span>
            </div>
          </div>
          <span>GAPLY_INTELLIGENCE_OS</span>
        </footer>
      </div>
      {/* Scanline overlay - dark mode only */}
      {theme === 'dark' && (
        <div
          className="scanline-overlay"
          style={{
            left: sidebarWidth,
            width: `calc(100% - ${sidebarWidth}px)`,
          }}
        />
      )}
    </div>
  );
};

export default DashboardLayout;
