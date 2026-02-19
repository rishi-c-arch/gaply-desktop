import React, { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import DashboardLayout from '../components/Layout/DashboardLayout';
import AccountCard from '../components/Dashboard/AccountCard';
import { useAuth } from '../contexts/AuthContext';

const SettingsPage: React.FC = () => {
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
    <DashboardLayout pageTitle="Settings">
      <div style={{ maxWidth: 420, margin: '0 auto' }}>
        <AccountCard />
      </div>
    </DashboardLayout>
  );
};

export default SettingsPage;
