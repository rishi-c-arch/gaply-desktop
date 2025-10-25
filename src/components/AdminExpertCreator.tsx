import React, { useState, useRef } from 'react';

interface ExpertProfile {
  id?: number;
  name: string;
  email: string;
  domains: string[];
  experience: number;
  publications: number;
  patents: number;
  bio: string;
  image_url: string;
  is_available: boolean;
}

interface ExpertDomain {
  id: number;
  name: string;
  category: string;
}

const AdminExpertCreator: React.FC = () => {
  const [profile, setProfile] = useState<ExpertProfile>({
    name: '',
    email: '',
    domains: [],
    experience: 0,
    publications: 0,
    patents: 0,
    bio: '',
    image_url: '',
    is_available: true
  });
  
  const [availableDomains, setAvailableDomains] = useState<ExpertDomain[]>([]);
  const [selectedDomain, setSelectedDomain] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [message, setMessage] = useState('');
  const [imagePreview, setImagePreview] = useState<string>('');
  const fileInputRef = useRef<HTMLInputElement>(null);

  // Load available domains on component mount
  React.useEffect(() => {
    fetchDomains();
  }, []);

  const fetchDomains = async () => {
    try {
      const response = await fetch('/api/expert/domains');
      if (response.ok) {
        const data = await response.json();
        setAvailableDomains(data.domains || []);
      }
    } catch (err) {
      console.error('Failed to fetch domains:', err);
      // Fallback to mock domains
      setAvailableDomains([
        { id: 1, name: 'Data Analysis & Statistics', category: 'Data Analysis' },
        { id: 2, name: 'Machine Learning / AI Engineer', category: 'Technology' },
        { id: 3, name: 'Research & Writing', category: 'Research' },
        { id: 4, name: 'Health & Medicine', category: 'Health' },
        { id: 5, name: 'Business & Economics', category: 'Business' }
      ]);
    }
  };

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => {
    const { name, value, type } = e.target;
    
    if (type === 'number') {
      setProfile(prev => ({
        ...prev,
        [name]: parseInt(value) || 0
      }));
    } else if (type === 'checkbox') {
      const checked = (e.target as HTMLInputElement).checked;
      setProfile(prev => ({
        ...prev,
        [name]: checked
      }));
    } else {
      setProfile(prev => ({
        ...prev,
        [name]: value
      }));
    }
  };

  const handleImageUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      // Create preview URL
      const previewUrl = URL.createObjectURL(file);
      setImagePreview(previewUrl);
      
      // For now, we'll use the file name as image_url
      // In production, you'd upload to a cloud service
      setProfile(prev => ({
        ...prev,
        image_url: file.name
      }));
    }
  };

  const addDomain = () => {
    if (selectedDomain && !profile.domains.includes(selectedDomain)) {
      setProfile(prev => ({
        ...prev,
        domains: [...prev.domains, selectedDomain]
      }));
      setSelectedDomain('');
    }
  };

  const removeDomain = (domainToRemove: string) => {
    setProfile(prev => ({
      ...prev,
      domains: prev.domains.filter(domain => domain !== domainToRemove)
    }));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsLoading(true);
    setMessage('');

    try {
      const response = await fetch('/api/admin/expert/profile', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${localStorage.getItem('admin_token')}` // You'll need to implement admin auth
        },
        body: JSON.stringify(profile)
      });

      if (response.ok) {
        setMessage('Expert profile created successfully!');
        // Reset form
        setProfile({
          name: '',
          email: '',
          domains: [],
          experience: 0,
          publications: 0,
          patents: 0,
          bio: '',
          image_url: '',
          is_available: true
        });
        setImagePreview('');
        if (fileInputRef.current) {
          fileInputRef.current.value = '';
        }
      } else {
        const errorData = await response.json();
        setMessage(`Error: ${errorData.error || 'Failed to create profile'}`);
      }
    } catch (err) {
      setMessage('Network error. Please try again.');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div style={{
      minHeight: '100vh',
      background: 'linear-gradient(135deg, #000000 0%, #0A0A0A 50%, #000000 100%)',
      color: '#ffffff',
      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
      padding: '40px 20px'
    }}>
      <div style={{
        maxWidth: '800px',
        margin: '0 auto',
        background: 'rgba(255, 255, 255, 0.05)',
        borderRadius: '20px',
        padding: '40px',
        backdropFilter: 'blur(20px)',
        WebkitBackdropFilter: 'blur(20px)',
        border: '1px solid rgba(255, 255, 255, 0.1)'
      }}>
        {/* Header */}
        <div style={{ textAlign: 'center', marginBottom: '40px' }}>
          <h1 style={{
            fontSize: '2.5rem',
            fontWeight: '700',
            marginBottom: '16px',
            background: 'linear-gradient(135deg, #007AFF 0%, #5856D6 100%)',
            WebkitBackgroundClip: 'text',
            WebkitTextFillColor: 'transparent',
            backgroundClip: 'text'
          }}>
            Create Expert Profile
          </h1>
          <p style={{
            fontSize: '1.1rem',
            color: 'rgba(255, 255, 255, 0.7)',
            lineHeight: '1.5'
          }}>
            Add new expert profiles to the platform
          </p>
        </div>

        {/* Message */}
        {message && (
          <div style={{
            padding: '16px',
            borderRadius: '12px',
            marginBottom: '32px',
            background: message.includes('Error') 
              ? 'rgba(255, 0, 0, 0.1)' 
              : 'rgba(0, 255, 0, 0.1)',
            border: `1px solid ${message.includes('Error') ? 'rgba(255, 0, 0, 0.3)' : 'rgba(0, 255, 0, 0.3)'}`,
            color: message.includes('Error') ? '#ff6b6b' : '#51cf66'
          }}>
            {message}
          </div>
        )}

        {/* Form */}
        <form onSubmit={handleSubmit}>
          <div style={{ display: 'grid', gap: '24px' }}>
            {/* Basic Information */}
            <div style={{
              background: 'rgba(255, 255, 255, 0.03)',
              borderRadius: '16px',
              padding: '24px',
              border: '1px solid rgba(255, 255, 255, 0.08)'
            }}>
              <h3 style={{
                fontSize: '1.3rem',
                fontWeight: '600',
                marginBottom: '20px',
                color: '#ffffff'
              }}>
                Basic Information
              </h3>
              
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '20px' }}>
                <div>
                  <label style={{
                    display: 'block',
                    fontSize: '14px',
                    fontWeight: '500',
                    color: 'rgba(255, 255, 255, 0.8)',
                    marginBottom: '8px'
                  }}>
                    Full Name *
                  </label>
                  <input
                    type="text"
                    name="name"
                    value={profile.name}
                    onChange={handleInputChange}
                    required
                    style={{
                      width: '100%',
                      padding: '12px 16px',
                      background: 'rgba(255, 255, 255, 0.05)',
                      border: '1px solid rgba(255, 255, 255, 0.1)',
                      borderRadius: '8px',
                      color: '#ffffff',
                      fontSize: '16px',
                      fontFamily: 'inherit',
                      outline: 'none',
                      transition: 'all 0.3s ease'
                    }}
                    onFocus={(e) => {
                      e.target.style.borderColor = '#007AFF';
                      e.target.style.background = 'rgba(255, 255, 255, 0.08)';
                    }}
                    onBlur={(e) => {
                      e.target.style.borderColor = 'rgba(255, 255, 255, 0.1)';
                      e.target.style.background = 'rgba(255, 255, 255, 0.05)';
                    }}
                  />
                </div>

                <div>
                  <label style={{
                    display: 'block',
                    fontSize: '14px',
                    fontWeight: '500',
                    color: 'rgba(255, 255, 255, 0.8)',
                    marginBottom: '8px'
                  }}>
                    Email *
                  </label>
                  <input
                    type="email"
                    name="email"
                    value={profile.email}
                    onChange={handleInputChange}
                    required
                    style={{
                      width: '100%',
                      padding: '12px 16px',
                      background: 'rgba(255, 255, 255, 0.05)',
                      border: '1px solid rgba(255, 255, 255, 0.1)',
                      borderRadius: '8px',
                      color: '#ffffff',
                      fontSize: '16px',
                      fontFamily: 'inherit',
                      outline: 'none',
                      transition: 'all 0.3s ease'
                    }}
                    onFocus={(e) => {
                      e.target.style.borderColor = '#007AFF';
                      e.target.style.background = 'rgba(255, 255, 255, 0.08)';
                    }}
                    onBlur={(e) => {
                      e.target.style.borderColor = 'rgba(255, 255, 255, 0.1)';
                      e.target.style.background = 'rgba(255, 255, 255, 0.05)';
                    }}
                  />
                </div>
              </div>

              <div style={{ marginTop: '20px' }}>
                <label style={{
                  display: 'block',
                  fontSize: '14px',
                  fontWeight: '500',
                  color: 'rgba(255, 255, 255, 0.8)',
                  marginBottom: '8px'
                }}>
                  Bio *
                </label>
                <textarea
                  name="bio"
                  value={profile.bio}
                  onChange={handleInputChange}
                  required
                  rows={4}
                  placeholder="Brief description of expertise and background..."
                  style={{
                    width: '100%',
                    padding: '12px 16px',
                    background: 'rgba(255, 255, 255, 0.05)',
                    border: '1px solid rgba(255, 255, 255, 0.1)',
                    borderRadius: '8px',
                    color: '#ffffff',
                    fontSize: '16px',
                    fontFamily: 'inherit',
                    outline: 'none',
                    resize: 'vertical',
                    transition: 'all 0.3s ease'
                  }}
                  onFocus={(e) => {
                    e.target.style.borderColor = '#007AFF';
                    e.target.style.background = 'rgba(255, 255, 255, 0.08)';
                  }}
                  onBlur={(e) => {
                    e.target.style.borderColor = 'rgba(255, 255, 255, 0.1)';
                    e.target.style.background = 'rgba(255, 255, 255, 0.05)';
                  }}
                />
              </div>
            </div>

            {/* Image Upload */}
            <div style={{
              background: 'rgba(255, 255, 255, 0.03)',
              borderRadius: '16px',
              padding: '24px',
              border: '1px solid rgba(255, 255, 255, 0.08)'
            }}>
              <h3 style={{
                fontSize: '1.3rem',
                fontWeight: '600',
                marginBottom: '20px',
                color: '#ffffff'
              }}>
                Profile Image
              </h3>

              <div style={{ display: 'flex', gap: '20px', alignItems: 'flex-start' }}>
                <div style={{ flex: 1 }}>
                  <input
                    ref={fileInputRef}
                    type="file"
                    accept="image/*"
                    onChange={handleImageUpload}
                    style={{ display: 'none' }}
                  />
                  <button
                    type="button"
                    onClick={() => fileInputRef.current?.click()}
                    style={{
                      width: '100%',
                      padding: '16px',
                      background: 'rgba(255, 255, 255, 0.05)',
                      border: '2px dashed rgba(255, 255, 255, 0.2)',
                      borderRadius: '12px',
                      color: 'rgba(255, 255, 255, 0.7)',
                      fontSize: '16px',
                      fontFamily: 'inherit',
                      cursor: 'pointer',
                      transition: 'all 0.3s ease'
                    }}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.borderColor = '#007AFF';
                      e.currentTarget.style.color = '#007AFF';
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.2)';
                      e.currentTarget.style.color = 'rgba(255, 255, 255, 0.7)';
                    }}
                  >
                    📷 Upload Profile Image
                  </button>
                </div>

                {imagePreview && (
                  <div style={{
                    width: '120px',
                    height: '120px',
                    borderRadius: '50%',
                    overflow: 'hidden',
                    border: '3px solid rgba(255, 255, 255, 0.2)'
                  }}>
                    <img
                      src={imagePreview}
                      alt="Preview"
                      style={{
                        width: '100%',
                        height: '100%',
                        objectFit: 'cover'
                      }}
                    />
                  </div>
                )}
              </div>
            </div>

            {/* Experience & Stats */}
            <div style={{
              background: 'rgba(255, 255, 255, 0.03)',
              borderRadius: '16px',
              padding: '24px',
              border: '1px solid rgba(255, 255, 255, 0.08)'
            }}>
              <h3 style={{
                fontSize: '1.3rem',
                fontWeight: '600',
                marginBottom: '20px',
                color: '#ffffff'
              }}>
                Experience & Statistics
              </h3>

              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: '20px' }}>
                <div>
                  <label style={{
                    display: 'block',
                    fontSize: '14px',
                    fontWeight: '500',
                    color: 'rgba(255, 255, 255, 0.8)',
                    marginBottom: '8px'
                  }}>
                    Experience (Years)
                  </label>
                  <input
                    type="number"
                    name="experience"
                    value={profile.experience}
                    onChange={handleInputChange}
                    min="0"
                    style={{
                      width: '100%',
                      padding: '12px 16px',
                      background: 'rgba(255, 255, 255, 0.05)',
                      border: '1px solid rgba(255, 255, 255, 0.1)',
                      borderRadius: '8px',
                      color: '#ffffff',
                      fontSize: '16px',
                      fontFamily: 'inherit',
                      outline: 'none',
                      transition: 'all 0.3s ease'
                    }}
                    onFocus={(e) => {
                      e.target.style.borderColor = '#007AFF';
                      e.target.style.background = 'rgba(255, 255, 255, 0.08)';
                    }}
                    onBlur={(e) => {
                      e.target.style.borderColor = 'rgba(255, 255, 255, 0.1)';
                      e.target.style.background = 'rgba(255, 255, 255, 0.05)';
                    }}
                  />
                </div>

                <div>
                  <label style={{
                    display: 'block',
                    fontSize: '14px',
                    fontWeight: '500',
                    color: 'rgba(255, 255, 255, 0.8)',
                    marginBottom: '8px'
                  }}>
                    Publications
                  </label>
                  <input
                    type="number"
                    name="publications"
                    value={profile.publications}
                    onChange={handleInputChange}
                    min="0"
                    style={{
                      width: '100%',
                      padding: '12px 16px',
                      background: 'rgba(255, 255, 255, 0.05)',
                      border: '1px solid rgba(255, 255, 255, 0.1)',
                      borderRadius: '8px',
                      color: '#ffffff',
                      fontSize: '16px',
                      fontFamily: 'inherit',
                      outline: 'none',
                      transition: 'all 0.3s ease'
                    }}
                    onFocus={(e) => {
                      e.target.style.borderColor = '#007AFF';
                      e.target.style.background = 'rgba(255, 255, 255, 0.08)';
                    }}
                    onBlur={(e) => {
                      e.target.style.borderColor = 'rgba(255, 255, 255, 0.1)';
                      e.target.style.background = 'rgba(255, 255, 255, 0.05)';
                    }}
                  />
                </div>

                <div>
                  <label style={{
                    display: 'block',
                    fontSize: '14px',
                    fontWeight: '500',
                    color: 'rgba(255, 255, 255, 0.8)',
                    marginBottom: '8px'
                  }}>
                    Patents
                  </label>
                  <input
                    type="number"
                    name="patents"
                    value={profile.patents}
                    onChange={handleInputChange}
                    min="0"
                    style={{
                      width: '100%',
                      padding: '12px 16px',
                      background: 'rgba(255, 255, 255, 0.05)',
                      border: '1px solid rgba(255, 255, 255, 0.1)',
                      borderRadius: '8px',
                      color: '#ffffff',
                      fontSize: '16px',
                      fontFamily: 'inherit',
                      outline: 'none',
                      transition: 'all 0.3s ease'
                    }}
                    onFocus={(e) => {
                      e.target.style.borderColor = '#007AFF';
                      e.target.style.background = 'rgba(255, 255, 255, 0.08)';
                    }}
                    onBlur={(e) => {
                      e.target.style.borderColor = 'rgba(255, 255, 255, 0.1)';
                      e.target.style.background = 'rgba(255, 255, 255, 0.05)';
                    }}
                  />
                </div>
              </div>
            </div>

            {/* Domains */}
            <div style={{
              background: 'rgba(255, 255, 255, 0.03)',
              borderRadius: '16px',
              padding: '24px',
              border: '1px solid rgba(255, 255, 255, 0.08)'
            }}>
              <h3 style={{
                fontSize: '1.3rem',
                fontWeight: '600',
                marginBottom: '20px',
                color: '#ffffff'
              }}>
                Research Domains
              </h3>

              <div style={{ display: 'flex', gap: '12px', marginBottom: '20px' }}>
                <select
                  value={selectedDomain}
                  onChange={(e) => setSelectedDomain(e.target.value)}
                  style={{
                    flex: 1,
                    padding: '12px 16px',
                    background: 'rgba(255, 255, 255, 0.05)',
                    border: '1px solid rgba(255, 255, 255, 0.1)',
                    borderRadius: '8px',
                    color: '#ffffff',
                    fontSize: '16px',
                    fontFamily: 'inherit',
                    outline: 'none'
                  }}
                >
                  <option value="">Select a domain...</option>
                  {availableDomains.map(domain => (
                    <option key={domain.id} value={domain.name}>
                      {domain.name} ({domain.category})
                    </option>
                  ))}
                </select>
                <button
                  type="button"
                  onClick={addDomain}
                  disabled={!selectedDomain}
                  style={{
                    padding: '12px 24px',
                    background: selectedDomain 
                      ? 'linear-gradient(135deg, #007AFF 0%, #5856D6 100%)' 
                      : 'rgba(255, 255, 255, 0.1)',
                    border: 'none',
                    borderRadius: '8px',
                    color: selectedDomain ? '#ffffff' : 'rgba(255, 255, 255, 0.4)',
                    fontSize: '16px',
                    fontFamily: 'inherit',
                    cursor: selectedDomain ? 'pointer' : 'not-allowed',
                    transition: 'all 0.3s ease'
                  }}
                >
                  Add Domain
                </button>
              </div>

              {profile.domains.length > 0 && (
                <div style={{
                  display: 'flex',
                  flexWrap: 'wrap',
                  gap: '8px'
                }}>
                  {profile.domains.map((domain, index) => (
                    <div
                      key={index}
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        gap: '8px',
                        padding: '8px 12px',
                        background: 'rgba(255, 255, 255, 0.1)',
                        borderRadius: '20px',
                        fontSize: '14px'
                      }}
                    >
                      <span>{domain}</span>
                      <button
                        type="button"
                        onClick={() => removeDomain(domain)}
                        style={{
                          background: 'none',
                          border: 'none',
                          color: 'rgba(255, 255, 255, 0.6)',
                          cursor: 'pointer',
                          fontSize: '16px',
                          padding: '0',
                          width: '20px',
                          height: '20px',
                          display: 'flex',
                          alignItems: 'center',
                          justifyContent: 'center'
                        }}
                      >
                        ×
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {/* Availability */}
            <div style={{
              background: 'rgba(255, 255, 255, 0.03)',
              borderRadius: '16px',
              padding: '24px',
              border: '1px solid rgba(255, 255, 255, 0.08)'
            }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                <input
                  type="checkbox"
                  name="is_available"
                  checked={profile.is_available}
                  onChange={handleInputChange}
                  style={{
                    width: '20px',
                    height: '20px',
                    accentColor: '#007AFF'
                  }}
                />
                <label style={{
                  fontSize: '16px',
                  fontWeight: '500',
                  color: '#ffffff'
                }}>
                  Expert is available for new projects
                </label>
              </div>
            </div>

            {/* Submit Button */}
            <div style={{ textAlign: 'center', marginTop: '32px' }}>
              <button
                type="submit"
                disabled={isLoading || !profile.name || !profile.email || !profile.bio}
                style={{
                  padding: '16px 48px',
                  background: (isLoading || !profile.name || !profile.email || !profile.bio)
                    ? 'rgba(255, 255, 255, 0.1)'
                    : 'linear-gradient(135deg, #007AFF 0%, #5856D6 100%)',
                  border: 'none',
                  borderRadius: '12px',
                  color: (isLoading || !profile.name || !profile.email || !profile.bio)
                    ? 'rgba(255, 255, 255, 0.4)'
                    : '#ffffff',
                  fontSize: '18px',
                  fontWeight: '600',
                  fontFamily: 'inherit',
                  cursor: (isLoading || !profile.name || !profile.email || !profile.bio)
                    ? 'not-allowed'
                    : 'pointer',
                  transition: 'all 0.3s ease',
                  minWidth: '200px'
                }}
                onMouseEnter={(e) => {
                  if (!isLoading && profile.name && profile.email && profile.bio) {
                    e.currentTarget.style.transform = 'translateY(-2px)';
                    e.currentTarget.style.boxShadow = '0 8px 32px rgba(0, 122, 255, 0.3)';
                  }
                }}
                onMouseLeave={(e) => {
                  if (!isLoading && profile.name && profile.email && profile.bio) {
                    e.currentTarget.style.transform = 'translateY(0)';
                    e.currentTarget.style.boxShadow = 'none';
                  }
                }}
              >
                {isLoading ? 'Creating Profile...' : 'Create Expert Profile'}
              </button>
            </div>
          </div>
        </form>
      </div>
    </div>
  );
};

export default AdminExpertCreator;
