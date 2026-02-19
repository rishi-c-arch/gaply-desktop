import React, { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { MessageCircle, Send, User, ThumbsUp, ChevronDown } from 'lucide-react';
import DashboardLayout from '../components/Layout/DashboardLayout';
import { useAuth } from '../contexts/AuthContext';

interface CommunityQuestion {
  id: string;
  author: string;
  avatar: string;
  question: string;
  date: string;
  likes: number;
  answers: number;
}

const MOCK_QUESTIONS: CommunityQuestion[] = [
  { id: '1', author: 'Priya M.', avatar: 'P', question: 'How do I export my PublishReady report as PDF?', date: '2 hours ago', likes: 12, answers: 3 },
  { id: '2', author: 'Rahul K.', avatar: 'R', question: 'Best practices for journal matching with a multidisciplinary paper?', date: '1 day ago', likes: 8, answers: 5 },
  { id: '3', author: 'Anita S.', avatar: 'A', question: 'DataMaestro: Can I re-run analysis after uploading a new dataset?', date: '2 days ago', likes: 5, answers: 2 },
  { id: '4', author: 'Vikram J.', avatar: 'V', question: 'What counts toward my PublishReady usage limit?', date: '3 days ago', likes: 14, answers: 4 },
];

const HelpCenterPage: React.FC = () => {
  const { isAuthenticated, user } = useAuth();
  const navigate = useNavigate();
  const [newQuestion, setNewQuestion] = useState('');
  const [questions, setQuestions] = useState<CommunityQuestion[]>(MOCK_QUESTIONS);
  const [expandedId, setExpandedId] = useState<string | null>(null);

  useEffect(() => {
    if (!isAuthenticated || !user) {
      navigate('/login');
    }
  }, [isAuthenticated, user, navigate]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!newQuestion.trim()) return;
    setQuestions((prev) => [
      {
        id: String(Date.now()),
        author: [user?.first_name, user?.last_name].filter(Boolean).join(' ') || 'You',
        avatar: (user?.first_name?.charAt(0) ?? 'U').toUpperCase(),
        question: newQuestion.trim(),
        date: 'Just now',
        likes: 0,
        answers: 0,
      },
      ...prev,
    ]);
    setNewQuestion('');
  };

  if (!isAuthenticated || !user) {
    return null;
  }

  return (
    <DashboardLayout pageTitle="Help Center">
      <div style={{ maxWidth: 720, margin: '0 auto' }}>
        {/* Hero */}
        <div
          style={{
            background: 'linear-gradient(135deg, rgba(59, 130, 246, 0.12) 0%, rgba(139, 92, 246, 0.08) 100%)',
            borderRadius: 16,
            border: '1px solid var(--dashboard-border)',
            padding: 32,
            marginBottom: 28,
            textAlign: 'center',
          }}
        >
          <div
            style={{
              width: 56,
              height: 56,
              borderRadius: 16,
              background: 'var(--dashboard-card-bg)',
              border: '1px solid var(--dashboard-border)',
              display: 'inline-flex',
              alignItems: 'center',
              justifyContent: 'center',
              marginBottom: 16,
            }}
          >
            <MessageCircle size={28} color="var(--dashboard-accent)" />
          </div>
          <h2 style={{ fontSize: 22, fontWeight: 600, color: 'var(--dashboard-text)', margin: '0 0 8px 0' }}>
            Community Q&A
          </h2>
          <p style={{ fontSize: 14, color: 'var(--dashboard-text-muted)', margin: 0, maxWidth: 420, marginLeft: 'auto', marginRight: 'auto' }}>
            Ask questions, share tips, and get answers from other researchers and the Gaply team.
          </p>
        </div>

        {/* Ask question */}
        <form
          onSubmit={handleSubmit}
          style={{
            background: 'var(--dashboard-card-bg)',
            borderRadius: 16,
            border: '1px solid var(--dashboard-border)',
            padding: 24,
            marginBottom: 24,
          }}
        >
          <label
            style={{
              display: 'block',
              fontSize: 14,
              fontWeight: 600,
              color: 'var(--dashboard-text)',
              marginBottom: 12,
            }}
          >
            Ask the community
          </label>
          <div style={{ display: 'flex', gap: 12 }}>
            <input
              type="text"
              value={newQuestion}
              onChange={(e) => setNewQuestion(e.target.value)}
              placeholder="e.g. How do I improve my publication chance score?"
              style={{
                flex: 1,
                padding: '14px 18px',
                fontSize: 15,
                color: 'var(--dashboard-text)',
                background: 'var(--dashboard-bg)',
                border: '1px solid var(--dashboard-border)',
                borderRadius: 12,
                outline: 'none',
                boxSizing: 'border-box',
              }}
            />
            <button
              type="submit"
              disabled={!newQuestion.trim()}
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: 8,
                padding: '14px 22px',
                fontSize: 14,
                fontWeight: 600,
                color: '#fff',
                background: !newQuestion.trim() ? 'var(--dashboard-text-muted)' : 'var(--dashboard-accent)',
                border: 'none',
                borderRadius: 12,
                cursor: !newQuestion.trim() ? 'not-allowed' : 'pointer',
                transition: 'all 150ms ease',
              }}
            >
              <Send size={18} />
              Post
            </button>
          </div>
        </form>

        {/* Questions list */}
        <div style={{ fontSize: 14, fontWeight: 600, color: 'var(--dashboard-text-muted)', marginBottom: 16 }}>
          Recent questions
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          {questions.map((q) => (
            <div
              key={q.id}
              style={{
                background: 'var(--dashboard-card-bg)',
                borderRadius: 14,
                border: '1px solid var(--dashboard-border)',
                overflow: 'hidden',
                transition: 'border-color 150ms ease',
              }}
            >
              <button
                type="button"
                onClick={() => setExpandedId(expandedId === q.id ? null : q.id)}
                style={{
                  width: '100%',
                  display: 'flex',
                  alignItems: 'flex-start',
                  gap: 14,
                  padding: 20,
                  background: 'transparent',
                  border: 'none',
                  cursor: 'pointer',
                  textAlign: 'left',
                }}
              >
                <div
                  style={{
                    width: 40,
                    height: 40,
                    borderRadius: 10,
                    background: 'linear-gradient(135deg, #3B82F6 0%, #8B5CF6 100%)',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    color: '#fff',
                    fontSize: 14,
                    fontWeight: 600,
                    flexShrink: 0,
                  }}
                >
                  {q.avatar}
                </div>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 15, fontWeight: 500, color: 'var(--dashboard-text)', marginBottom: 6 }}>
                    {q.question}
                  </div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 16, flexWrap: 'wrap' }}>
                    <span style={{ fontSize: 13, color: 'var(--dashboard-text-muted)' }}>{q.author} · {q.date}</span>
                    <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6, fontSize: 12, color: 'var(--dashboard-text-muted)' }}>
                      <ThumbsUp size={14} /> {q.likes} · {q.answers} answers
                    </span>
                  </div>
                </div>
                <ChevronDown
                  size={20}
                  color="var(--dashboard-text-muted)"
                  style={{ flexShrink: 0, transform: expandedId === q.id ? 'rotate(180deg)' : 'none', transition: 'transform 150ms ease' }}
                />
              </button>
              {expandedId === q.id && (
                <div
                  style={{
                    padding: '0 20px 20px 74px',
                    fontSize: 14,
                    color: 'var(--dashboard-text-muted)',
                    lineHeight: 1.6,
                  }}
                >
                  <p style={{ margin: 0 }}>Answers and discussion appear here. (Community answers can be wired to a backend later.)</p>
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
    </DashboardLayout>
  );
};

export default HelpCenterPage;
