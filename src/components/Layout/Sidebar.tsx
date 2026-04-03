import React from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import { ChevronsLeft } from 'lucide-react';
import { useAuth } from '../../contexts/AuthContext';

const NAV_ITEMS: Array<{
  id: string;
  label: string;
  icon: string;
  path: string;
  premium?: boolean;
  section: 'primary' | 'operations';
}> = [
  { id: 'overview', label: 'Overview', icon: 'grid_view', path: '/dashboard', section: 'primary' },
  { id: 'publishready', label: 'PublishReady', icon: 'verified', path: '/dashboard/publishready', premium: true, section: 'primary' },
  { id: 'datamaestro', label: 'DataMaestro', icon: 'database', path: '/dashboard/datamaestro', premium: true, section: 'primary' },
  { id: 'journal-verify', label: 'Journal Verification', icon: 'fact_check', path: '/dashboard/journal-verify', premium: true, section: 'primary' },
  { id: 'research-deep-analysis', label: 'Research Deep Analysis', icon: 'menu_book', path: '/dashboard/research-deep-analysis', premium: true, section: 'primary' },
  { id: 'projects', label: 'Projects', icon: 'folder_managed', path: '/dashboard/projects', section: 'primary' },
  { id: 'usage', label: 'Usage', icon: 'monitoring', path: '/dashboard/usage', section: 'operations' },
  { id: 'billing', label: 'Billing & Invoices', icon: 'receipt_long', path: '/dashboard/billing', section: 'operations' },
  { id: 'settings', label: 'Settings', icon: 'settings', path: '/dashboard/settings', section: 'operations' },
];

interface SidebarProps {
  collapsed: boolean;
  onCollapsedChange: (collapsed: boolean) => void;
  mobileOpen?: boolean;
  onMobileClose?: () => void;
  isMobile?: boolean;
}

const Sidebar: React.FC<SidebarProps> = ({
  collapsed,
  onCollapsedChange,
  mobileOpen = false,
  onMobileClose = () => {},
  isMobile = false,
}) => {
  const navigate = useNavigate();
  const location = useLocation();
  const { logout } = useAuth();

  const handleNavClick = (path: string) => {
    navigate(path);
    if (isMobile) onMobileClose();
  };

  const handleLogout = () => {
    logout();
    if (isMobile) onMobileClose();
  };

  const width = isMobile ? 260 : (collapsed ? 64 : 260);
  const primaryItems = NAV_ITEMS.filter((i) => i.section === 'primary');
  const operationsItems = NAV_ITEMS.filter((i) => i.section === 'operations');

  return (
    <aside
      className="dashboard-hud"
      style={{
        width,
        minWidth: width,
        background: 'var(--dashboard-sidebar-bg)',
        borderRight: '1px solid var(--dashboard-border)',
        height: '100vh',
        position: 'fixed',
        left: 0,
        top: 0,
        zIndex: 40,
        transition: 'width 150ms ease, transform 150ms ease',
        display: 'flex',
        flexDirection: 'column',
        transform: isMobile && !mobileOpen ? 'translateX(-100%)' : 'translateX(0)',
        fontFamily: "'Space Grotesk', sans-serif",
      }}
    >
      <div style={{ padding: 24, display: 'flex', alignItems: 'center', gap: 12, borderBottom: '1px solid var(--dashboard-border)' }}>
        {!collapsed && (
          <div style={{ display: 'flex', flexDirection: 'column' }}>
            <span style={{ fontSize: 18, fontWeight: 700, letterSpacing: '0.1em', color: 'var(--dashboard-text)' }}>
              GAPLY.IN
            </span>
            <span
              style={{
                fontSize: 10,
                color: 'var(--dashboard-accent)',
                textTransform: 'uppercase',
                letterSpacing: '0.2em',
                fontWeight: 500,
              }}
            >
              Intelligence Dashboard
            </span>
          </div>
        )}
        {!isMobile && (
          <button
            onClick={() => onCollapsedChange(!collapsed)}
            style={{
              marginLeft: 'auto',
              background: 'transparent',
              border: 'none',
              color: 'var(--dashboard-text-muted)',
              cursor: 'pointer',
              padding: 4,
            }}
          >
            <ChevronsLeft size={18} style={{ transform: collapsed ? 'rotate(180deg)' : 'none' }} />
          </button>
        )}
      </div>

      <nav style={{ flex: 1, padding: 16, marginTop: 16, overflowY: 'auto' }}>
        <div style={{ marginBottom: 32 }}>
          <p style={{ fontSize: 10, color: 'var(--dashboard-text-muted)', textTransform: 'uppercase', letterSpacing: '0.15em', marginBottom: 16, paddingLeft: 12 }}>
            Primary Nodes
          </p>
          {primaryItems.map((item) => {
            const isActive =
              location.pathname === item.path ||
              (item.id === 'overview' && (location.pathname === '/account' || location.pathname === '/my-account'));
            return (
              <button
                key={item.id}
                onClick={() => handleNavClick(item.path)}
                style={{
                  width: '100%',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 12,
                  padding: '10px 12px',
                  fontSize: 14,
                  fontWeight: 500,
                  letterSpacing: '0.02em',
                  background: isActive ? 'var(--dashboard-sidebar-active-bg)' : 'transparent',
                  border: 'none',
                  borderLeft: `2px solid ${isActive ? 'var(--dashboard-accent)' : 'transparent'}`,
                  color: isActive ? 'var(--dashboard-accent)' : 'var(--dashboard-text-muted)',
                  cursor: 'pointer',
                  textAlign: 'left',
                  transition: 'all 150ms ease',
                }}
              >
                <span
                  className="material-symbols-outlined"
                  style={{ fontSize: 20, color: isActive ? 'var(--dashboard-accent)' : 'inherit' }}
                >
                  {item.icon}
                </span>
                {!collapsed && (
                  <span style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                    {item.label}
                    {item.premium && (
                      <span
                        style={{
                          fontSize: 10,
                          fontWeight: 600,
                          color: 'var(--dashboard-accent)',
                          background: 'var(--dashboard-sidebar-active-bg)',
                          padding: '2px 6px',
                          borderRadius: 4,
                        }}
                      >
                        Pro
                      </span>
                    )}
                  </span>
                )}
              </button>
            );
          })}
        </div>
        <div>
          <p style={{ fontSize: 10, color: 'var(--dashboard-text-muted)', textTransform: 'uppercase', letterSpacing: '0.15em', marginBottom: 16, paddingLeft: 12 }}>
            Operations
          </p>
          {operationsItems.map((item) => {
            const isActive = location.pathname === item.path;
            return (
              <button
                key={item.id}
                onClick={() => handleNavClick(item.path)}
                style={{
                  width: '100%',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 12,
                  padding: '10px 12px',
                  fontSize: 14,
                  fontWeight: 500,
                  letterSpacing: '0.02em',
                  background: isActive ? 'var(--dashboard-sidebar-active-bg)' : 'transparent',
                  border: 'none',
                  borderLeft: `2px solid ${isActive ? 'var(--dashboard-accent)' : 'transparent'}`,
                  color: isActive ? 'var(--dashboard-accent)' : 'var(--dashboard-text-muted)',
                  cursor: 'pointer',
                  textAlign: 'left',
                  transition: 'all 150ms ease',
                }}
              >
                <span
                  className="material-symbols-outlined"
                  style={{ fontSize: 20, color: isActive ? 'var(--dashboard-accent)' : 'inherit' }}
                >
                  {item.icon}
                </span>
                {!collapsed && <span>{item.label}</span>}
              </button>
            );
          })}
        </div>
      </nav>

      <div style={{ padding: 16, borderTop: '1px solid var(--dashboard-border)' }}>
        <button
          onClick={() => handleNavClick('/dashboard/settings')}
          style={{
            width: '100%',
            display: 'flex',
            alignItems: 'center',
            gap: 12,
            padding: '10px 12px',
            fontSize: 14,
            fontWeight: 500,
            background: 'transparent',
            border: 'none',
            color: 'var(--dashboard-danger)',
            cursor: 'pointer',
            textAlign: 'left',
            transition: 'background 150ms ease',
          }}
        >
          <span className="material-symbols-outlined" style={{ fontSize: 20 }}>no_accounts</span>
          {!collapsed && <span>Delete Account</span>}
        </button>
        <button
          onClick={handleLogout}
          style={{
            width: '100%',
            display: 'flex',
            alignItems: 'center',
            gap: 12,
            padding: '10px 12px',
            fontSize: 14,
            fontWeight: 500,
            background: 'transparent',
            border: 'none',
            color: 'var(--dashboard-text-muted)',
            cursor: 'pointer',
            textAlign: 'left',
            transition: 'all 150ms ease',
          }}
        >
          <span className="material-symbols-outlined" style={{ fontSize: 20 }}>logout</span>
          {!collapsed && <span>Logout</span>}
        </button>
      </div>
    </aside>
  );
};

export default Sidebar;
