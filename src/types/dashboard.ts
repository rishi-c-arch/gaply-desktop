export interface NavItem {
  id: string;
  label: string;
  icon: string;
  active?: boolean;
}

export interface StatsCard {
  id: string;
  title: string;
  value: string;
  showInfo?: boolean;
}

export interface Project {
  id: string;
  name: string;
  date: string;
  status: 'complete' | 'running' | string;
}

export interface AccountMenuItem {
  id: string;
  label: string;
  sublabel?: string;
  icon?: string;
  highlight?: boolean;
}

export interface SupportMenuItem {
  id: string;
  label: string;
  icon: string;
}

export interface ChartDataPoint {
  month: string;
  value?: number;
  value2?: number;
  publishready?: number;
  datamaestro?: number;
  journal_check?: number;
  research_deep?: number;
}

export interface BillingTransaction {
  id: string;
  date: string;
  description: string;
  amount: string;
  status: 'billed';
}
