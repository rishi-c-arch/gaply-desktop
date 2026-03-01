import React from 'react';
import {
  LayoutDashboard,
  Folder,
  Clock,
  Settings,
  ChevronsLeft,
  FileText,
  BarChart2,
} from 'lucide-react';
import { useNavigate, useLocation } from 'react-router-dom';

const NAV_ITEMS: Array<{
  id: string;
  label: string;
  icon: React.ComponentType<{ size?: number; style?: React.CSSProperties }>;
  path: string;
  premium?: boolean;
}> = [
  { id: 'overview', label: 'Overview', icon: LayoutDashboard, path: '/dashboard' },
  { id: 'publishready', label: 'PublishReady', icon: FileText, path: '/dashboard/publishready', premium: true },
  { id: 'datamaestro', label: 'DataMaestro', icon: BarChart2, path: '/datamaestro-pro', premium: true },
  { id: 'projects', label: 'Projects', icon: Folder, path: '/dashboard/projects' },
  { id: 'usage', label: 'Usage', icon: Clock, path: '/dashboard/usage' },
  { id: 'settings', label: 'Settings', icon: Settings, path: '/dashboard/settings' },
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
  const handleNavClick = (path: string, _premium?: boolean) => {
    navigate(path);
    if (isMobile) onMobileClose();
  };

  const width = isMobile ? 260 : (collapsed ? 64 : 260);

  return (
    <aside
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
      }}
    >
      <div style={{ padding: '16px', display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <div
            style={{
              width: 32,
              height: 32,
              borderRadius: 8,
              background: 'linear-gradient(135deg, #3B82F6 0%, #2563EB 100%)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              color: 'white',
              fontWeight: 700,
              fontSize: 16,
            }}
          >
            S
          </div>
          {!collapsed && (
            <span style={{ color: 'var(--dashboard-text)', fontSize: 16, fontWeight: 600 }}>gaply.in</span>
          )}
        </div>
        {!isMobile && (
          <button
            onClick={() => onCollapsedChange(!collapsed)}
            style={{
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

      <nav style={{ flex: 1, padding: '12px 0', marginTop: 8 }}>
        {NAV_ITEMS.map((item) => {
          const isActive = location.pathname === item.path || (item.id === 'overview' && (location.pathname === '/account' || location.pathname === '/my-account'));
          const isPremium = item.premium === true;
          const Icon = item.icon;
          return (
            <button
              key={item.id}
              onClick={() => handleNavClick(item.path, isPremium)}
              style={{
                width: '100%',
                display: 'flex',
                alignItems: 'center',
                gap: 12,
                padding: collapsed ? '12px 16px' : '12px 20px',
                background: isActive ? 'var(--dashboard-sidebar-active-bg)' : 'transparent',
                border: 'none',
                borderLeft: `3px solid ${isActive ? 'var(--dashboard-accent)' : 'transparent'}`,
                color: isActive ? 'var(--dashboard-text)' : 'var(--dashboard-text-muted)',
                cursor: isPremium ? 'default' : 'pointer',
                fontSize: 14,
                fontWeight: 500,
                textAlign: 'left',
                transition: 'all 150ms ease',
                opacity: isPremium ? 0.85 : 1,
              }}
              title={isPremium ? 'Coming soon – Premium feature' : undefined}
            >
              <Icon size={20} style={{ flexShrink: 0 }} />
              {!collapsed && (
                <span style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                  {item.label}
                  {isPremium && (
                    <span
                      style={{
                        fontSize: 10,
                        fontWeight: 600,
                        color: 'var(--dashboard-accent)',
                        background: 'rgba(59, 130, 246, 0.15)',
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
      </nav>
    </aside>
  );
};

export default Sidebar;
