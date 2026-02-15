import React, { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import DashboardLayout from '../components/Layout/DashboardLayout';
import ChartCard from '../components/Dashboard/ChartCard';
import StatsCard from '../components/Dashboard/StatsCard';
import ProjectsTable from '../components/Dashboard/ProjectsTable';
import AccountCard from '../components/Dashboard/AccountCard';
import { statsCards } from '../data/mockData';
import { useAuth } from '../contexts/AuthContext';

const Overview: React.FC = () => {
  const { isAuthenticated, user } = useAuth();
  const navigate = useNavigate();

  useEffect(() => {
    if (!isAuthenticated || !user) {
      navigate('/login');
    }
  }, [isAuthenticated, user, navigate]);

  if (!isAuthenticated || !user) {
    return null;
  }
  return (
    <DashboardLayout>
      <div className="grid grid-cols-1 lg:grid-cols-[1fr_380px] gap-6 max-w-[1400px] mx-auto">
        <div className="min-w-0">
          <ChartCard />
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-6 mb-6">
            {statsCards.map((card) => (
              <StatsCard
                key={card.id}
                title={card.title}
                value={card.value}
                showInfo={card.showInfo}
              />
            ))}
          </div>
          <ProjectsTable />
        </div>
        <div className="min-w-0">
          <AccountCard />
        </div>
      </div>
    </DashboardLayout>
  );
};

export default Overview;
