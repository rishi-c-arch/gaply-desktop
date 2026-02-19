import type { NavItem, AccountMenuItem, SupportMenuItem } from '../types/dashboard';

export const navigationItems: NavItem[] = [
  { id: 'overview', label: 'Overview', icon: 'LayoutDashboard', active: true },
  { id: 'projects', label: 'Projects', icon: 'Folder' },
  { id: 'usage', label: 'Usage', icon: 'Clock' },
  { id: 'settings', label: 'Settings', icon: 'Settings' },
];

export const accountMenuItems: AccountMenuItem[] = [
  { id: 'plan', label: 'Plan', sublabel: 'Gaply Premium (Upgrade)', highlight: true },
  { id: 'billing', label: 'Billing', icon: 'CreditCard' },
  { id: 'profile', label: 'Profile', icon: 'User' },
];

export const supportMenuItems: SupportMenuItem[] = [
  { id: 'contact', label: 'Contact Support', icon: 'Mail' },
  { id: 'help', label: 'Help Center', icon: 'HelpCircle' },
];
