import React, { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { User, Award, Save } from 'lucide-react';
import DashboardLayout from '../components/Layout/DashboardLayout';
import { useAuth } from '../contexts/AuthContext';

const PERFORMANCE_SCORE = 87; // 0–100

const ProfilePage: React.FC = () => {
  const { isAuthenticated, user } = useAuth();
  const navigate = useNavigate();
  const [name, setName] = useState('');
  const [profession, setProfession] = useState('Research Scholar');
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    if (!isAuthenticated || !user) {
      navigate('/login');
    }
  }, [isAuthenticated, user, navigate]);

  useEffect(() => {
    const full = user ? [user.first_name, user.last_name].filter(Boolean).join(' ') : '';
    if (full) setName(full);
  }, [user?.first_name, user?.last_name]);

  const handleSave = (e: React.FormEvent) => {
    e.preventDefault();
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  if (!isAuthenticated || !user) {
    return null;
  }

  const scoreColor =
    PERFORMANCE_SCORE >= 80 ? 'var(--dashboard-success)' : PERFORMANCE_SCORE >= 50 ? 'var(--dashboard-warning)' : 'var(--dashboard-error, #ef4444)';

  return (
    <DashboardLayout pageTitle="Profile">
      <div style={{ maxWidth: 560, margin: '0 auto' }}>
        {/* Performance score card */}
        <div
          style={{
            background: 'var(--dashboard-card-bg)',
            borderRadius: 16,
            border: '1px solid var(--dashboard-border)',
            padding: 28,
            marginBottom: 24,
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 20 }}>
            <div
              style={{
                width: 44,
                height: 44,
                borderRadius: 12,
                background: 'linear-gradient(135deg, rgba(59, 130, 246, 0.15) 0%, rgba(139, 92, 246, 0.15) 100%)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
              }}
            >
              <Award size={22} color="var(--dashboard-accent)" />
            </div>
            <div>
              <h2 style={{ fontSize: 18, fontWeight: 600, color: 'var(--dashboard-text)', margin: '0 0 4px 0' }}>
                Performance score
              </h2>
              <p style={{ fontSize: 13, color: 'var(--dashboard-text-muted)', margin: 0 }}>
                Based on your usage and result quality
              </p>
            </div>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 24 }}>
            <div
              style={{
                width: 120,
                height: 120,
                borderRadius: '50%',
                border: `4px solid var(--dashboard-border)`,
                borderTopColor: scoreColor,
                borderRightColor: scoreColor,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                transform: 'rotate(-45deg)',
              }}
            >
              <span
                style={{
                  transform: 'rotate(45deg)',
                  fontSize: 28,
                  fontWeight: 700,
                  color: 'var(--dashboard-text)',
                }}
              >
                {PERFORMANCE_SCORE}
              </span>
            </div>
            <div>
              <div style={{ fontSize: 15, fontWeight: 600, color: 'var(--dashboard-text)', marginBottom: 4 }}>
                {PERFORMANCE_SCORE >= 80 ? 'Excellent' : PERFORMANCE_SCORE >= 50 ? 'Good' : 'Needs improvement'}
              </div>
              <p style={{ fontSize: 13, color: 'var(--dashboard-text-muted)', margin: 0 }}>
                You're in the top tier of users. Keep it up!
              </p>
            </div>
          </div>
        </div>

        {/* Edit profile form */}
        <form
          onSubmit={handleSave}
          style={{
            background: 'var(--dashboard-card-bg)',
            borderRadius: 16,
            border: '1px solid var(--dashboard-border)',
            padding: 28,
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 24 }}>
            <div
              style={{
                width: 44,
                height: 44,
                borderRadius: 12,
                background: 'var(--dashboard-sidebar-active-bg)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
              }}
            >
              <User size={22} color="var(--dashboard-text-muted)" />
            </div>
            <div>
              <h2 style={{ fontSize: 18, fontWeight: 600, color: 'var(--dashboard-text)', margin: '0 0 4px 0' }}>
                Personal details
              </h2>
              <p style={{ fontSize: 13, color: 'var(--dashboard-text-muted)', margin: 0 }}>
                Update your name and profession
              </p>
            </div>
          </div>

          <div style={{ marginBottom: 20 }}>
            <label
              style={{
                display: 'block',
                fontSize: 13,
                fontWeight: 500,
                color: 'var(--dashboard-text-muted)',
                marginBottom: 8,
              }}
            >
              Name
            </label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Your name"
              style={{
                width: '100%',
                padding: '12px 16px',
                fontSize: 15,
                color: 'var(--dashboard-text)',
                background: 'var(--dashboard-bg)',
                border: '1px solid var(--dashboard-border)',
                borderRadius: 10,
                outline: 'none',
                boxSizing: 'border-box',
              }}
            />
          </div>

          <div style={{ marginBottom: 24 }}>
            <label
              style={{
                display: 'block',
                fontSize: 13,
                fontWeight: 500,
                color: 'var(--dashboard-text-muted)',
                marginBottom: 8,
              }}
            >
              Profession
            </label>
            <input
              type="text"
              value={profession}
              onChange={(e) => setProfession(e.target.value)}
              placeholder="e.g. Research Scholar, PhD Student"
              style={{
                width: '100%',
                padding: '12px 16px',
                fontSize: 15,
                color: 'var(--dashboard-text)',
                background: 'var(--dashboard-bg)',
                border: '1px solid var(--dashboard-border)',
                borderRadius: 10,
                outline: 'none',
                boxSizing: 'border-box',
              }}
            />
          </div>

          <button
            type="submit"
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              gap: 8,
              padding: '12px 24px',
              fontSize: 14,
              fontWeight: 600,
              color: '#fff',
              background: saved ? 'var(--dashboard-success)' : 'var(--dashboard-accent)',
              border: 'none',
              borderRadius: 10,
              cursor: 'pointer',
              transition: 'all 150ms ease',
            }}
          >
            <Save size={18} />
            {saved ? 'Saved' : 'Save changes'}
          </button>
        </form>
      </div>
    </DashboardLayout>
  );
};

export default ProfilePage;
