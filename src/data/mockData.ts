import type { NavItem, StatsCard, Project, AccountMenuItem, SupportMenuItem, ChartDataPoint } from '../types/dashboard';

export const navigationItems: NavItem[] = [
  { id: 'overview', label: 'Overview', icon: 'LayoutDashboard', active: true },
  { id: 'projects', label: 'Projects', icon: 'Folder' },
  { id: 'usage', label: 'Usage', icon: 'Clock' },
  { id: 'settings', label: 'Settings', icon: 'Settings' },
];

export const statsCards: StatsCard[] = [
  { id: 'publishready', title: 'PublishReady Uses', value: '2/2', showInfo: true },
  { id: 'datamaestro', title: 'DataMaestro Uses', value: '1/1', showInfo: true },
  { id: 'projects', title: 'Total Projects', value: '3' },
];

export const projects: Project[] = [
  { id: '1', name: 'AI Research Paper', date: 'Jan 28, 2023', status: 'complete' },
  { id: '2', name: 'Data Analysis V1', date: 'Jan 24, 2023', status: 'complete' },
  { id: '3', name: 'Market Trends Report', date: 'Jan 21, 2023', status: 'running' },
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

export const chartData: ChartDataPoint[] = [
  { month: 'Jan', value: 0.5, value2: 0.3 },
  { month: 'Feb', value: 0.8, value2: 0.5 },
  { month: 'Mar', value: 1.2, value2: 0.8 },
  { month: 'Apr', value: 1.0, value2: 1.0 },
  { month: 'May', value: 1.5, value2: 1.2 },
  { month: 'Jun', value: 1.8, value2: 1.4 },
  { month: 'Jul', value: 2.0, value2: 1.6 },
  { month: 'Aug', value: 2.2, value2: 1.8 },
  { month: 'Sep', value: 2.0, value2: 2.0 },
  { month: 'Oct', value: 2.3, value2: 2.1 },
  { month: 'Nov', value: 2.5, value2: 2.3 },
  { month: 'Dec', value: 2.2, value2: 2.0 },
];
