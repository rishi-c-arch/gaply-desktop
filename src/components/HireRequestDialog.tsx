import React, { useState } from 'react';

interface ExpertProfile {
  id: number;
  name: string;
  email: string;
  domains: string[];
  experience: number;
  publications: number;
  patents: number;
  bio: string;
  image_url: string;
  is_available: boolean;
  created_at: string;
}

interface HireRequestDialogProps {
  expert: ExpertProfile;
  isOpen: boolean;
  onClose: () => void;
}

const HireRequestDialog: React.FC<HireRequestDialogProps> = ({ expert, isOpen, onClose }) => {
  const [formData, setFormData] = useState({
    userType: '',
    domain: '',
    purpose: '',
    phoneNumber: '',
    email: '',
    additionalDetails: ''
  });
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isSubmitted, setIsSubmitted] = useState(false);
  const [errors, setErrors] = useState<Record<string, string>>({});

  const userTypes = [
    { value: 'phd', label: 'PhD Student', description: 'Pursuing doctoral research and need expert guidance' },
    { value: 'individual', label: 'Individual Researcher', description: 'Independent researcher seeking collaboration' },
    { value: 'corporate', label: 'Corporate Researcher', description: 'Industry professional requiring specialized expertise' },
    { value: 'masters', label: 'Master\'s Student', description: 'Graduate student needing research assistance' }
  ];

  const domains = [
    'Data Analysis & Statistics',
    'Machine Learning & AI',
    'Medical Research',
    'Business & Economics',
    'Computer Science',
    'Environmental Science',
    'Psychology & Social Sciences',
    'Engineering & Technology',
    'Public Health',
    'Education & Learning',
    'Other'
  ];

  const purposes = [
    'Thesis Writing & Guidance',
    'Research Paper Development',
    'Statistical Analysis',
    'Literature Review',
    'Data Collection & Analysis',
    'Methodology Design',
    'Publication Support',
    'Grant Writing',
    'Conference Presentation',
    'Peer Review',
    'Other'
  ];

  const validateForm = () => {
    const newErrors: Record<string, string> = {};

    if (!formData.userType) {
      newErrors.userType = 'Please select your research background';
    }
    if (!formData.domain) {
      newErrors.domain = 'Please specify your research domain';
    }
    if (!formData.purpose) {
      newErrors.purpose = 'Please describe your project purpose';
    }
    if (!formData.phoneNumber) {
      newErrors.phoneNumber = 'Phone number is required';
    } else if (!/^[+]?[1-9][\d]{0,15}$/.test(formData.phoneNumber.replace(/[\s\-+()]/g, ''))) {
      newErrors.phoneNumber = 'Please enter a valid phone number';
    }
    if (!formData.email) {
      newErrors.email = 'Email address is required';
    } else if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(formData.email)) {
      newErrors.email = 'Please enter a valid email address';
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    
    if (!validateForm()) {
      return;
    }

    setIsSubmitting(true);

    try {
      // Submit the hire request to backend
      const response = await fetch('http://localhost:8080/api/hire-requests', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          expertId: expert.id,
          expertName: expert.name,
          userType: formData.userType,
          domain: formData.domain,
          purpose: formData.purpose,
          phoneNumber: formData.phoneNumber,
          email: formData.email,
          additionalDetails: formData.additionalDetails,
          submittedAt: new Date().toISOString()
        }),
      });

      if (response.ok) {
        setIsSubmitted(true);
        // Reset form after successful submission
        setTimeout(() => {
          setFormData({
            userType: '',
            domain: '',
            purpose: '',
            phoneNumber: '',
            email: '',
            additionalDetails: ''
          });
          setIsSubmitted(false);
          onClose();
        }, 3000);
      } else {
        throw new Error('Failed to submit request');
      }
    } catch (error) {
      console.error('Error submitting hire request:', error);
      setErrors({ submit: 'Failed to submit request. Please try again.' });
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleInputChange = (field: string, value: string) => {
    setFormData(prev => ({ ...prev, [field]: value }));
    // Clear error when user starts typing
    if (errors[field]) {
      setErrors(prev => ({ ...prev, [field]: '' }));
    }
  };

  if (!isOpen) return null;

  return (
    <div style={{
      position: 'fixed',
      top: 0,
      left: 0,
      right: 0,
      bottom: 0,
      backgroundColor: 'rgba(0, 0, 0, 0.8)',
      backdropFilter: 'blur(20px)',
      WebkitBackdropFilter: 'blur(20px)',
      zIndex: 10000,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      padding: '20px'
    }}>
      <div style={{
        backgroundColor: '#000000',
        borderRadius: '24px',
        border: '1px solid rgba(255, 255, 255, 0.1)',
        maxWidth: '600px',
        width: '100%',
        maxHeight: '90vh',
        overflowY: 'auto',
        boxShadow: '0 32px 80px rgba(0, 0, 0, 0.6)',
        position: 'relative'
      }}>
        {/* Header */}
        <div style={{
          padding: '32px 32px 24px 32px',
          borderBottom: '1px solid rgba(255, 255, 255, 0.1)',
          position: 'relative'
        }}>
          <button
            onClick={onClose}
            style={{
              position: 'absolute',
              top: '24px',
              right: '24px',
              background: 'rgba(255, 255, 255, 0.1)',
              border: 'none',
              borderRadius: '12px',
              width: '40px',
              height: '40px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              cursor: 'pointer',
              transition: 'all 0.3s ease',
              color: '#ffffff',
              fontSize: '18px'
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = 'rgba(255, 255, 255, 0.2)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'rgba(255, 255, 255, 0.1)';
            }}
          >
            ×
          </button>

          <div style={{
            display: 'flex',
            alignItems: 'center',
            marginBottom: '20px'
          }}>
            <div style={{
              width: '60px',
              height: '60px',
              borderRadius: '50%',
              background: 'rgba(255, 255, 255, 0.1)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              fontSize: '24px',
              fontWeight: '700',
              color: '#ffffff',
              marginRight: '20px',
              border: '2px solid rgba(255, 255, 255, 0.2)'
            }}>
              {expert.name.split(' ').map(n => n[0]).join('').toUpperCase()}
            </div>
            <div>
              <h2 style={{
                fontSize: '24px',
                fontWeight: '700',
                color: '#ffffff',
                margin: '0 0 8px 0',
                letterSpacing: '-0.01em'
              }}>
                Hire {expert.name}
              </h2>
              <p style={{
                fontSize: '16px',
                color: 'rgba(255, 255, 255, 0.7)',
                margin: '0',
                fontWeight: '400'
              }}>
                {expert.experience} years experience • {expert.publications} publications
              </p>
            </div>
          </div>

          <p style={{
            fontSize: '16px',
            color: 'rgba(255, 255, 255, 0.8)',
            lineHeight: '1.5',
            margin: '0'
          }}>
            Connect with our expert through Gaply's professional matching service. 
            Our team will facilitate the connection and ensure you get the best research support.
          </p>
        </div>

        {/* Success Message */}
        {isSubmitted && (
          <div style={{
            padding: '32px',
            textAlign: 'center',
            background: 'rgba(0, 255, 0, 0.1)',
            border: '1px solid rgba(0, 255, 0, 0.3)',
            margin: '20px',
            borderRadius: '16px'
          }}>
            <div style={{
              fontSize: '48px',
              marginBottom: '16px'
            }}>
              ✅
            </div>
            <h3 style={{
              fontSize: '20px',
              fontWeight: '600',
              color: '#51cf66',
              marginBottom: '12px'
            }}>
              Request Submitted Successfully!
            </h3>
            <p style={{
              fontSize: '16px',
              color: 'rgba(255, 255, 255, 0.8)',
              lineHeight: '1.5'
            }}>
              Our Gaply team will review your request and connect you with {expert.name} within 24 hours. 
              You'll receive a confirmation email shortly.
            </p>
          </div>
        )}

        {/* Form */}
        {!isSubmitted && (
          <form onSubmit={handleSubmit} style={{ padding: '32px' }}>
            {/* Research Background */}
            <div style={{ marginBottom: '32px' }}>
              <label style={{
                display: 'block',
                fontSize: '16px',
                fontWeight: '600',
                color: '#ffffff',
                marginBottom: '16px'
              }}>
                Research Background *
              </label>
              <div style={{ display: 'grid', gap: '12px' }}>
                {userTypes.map((type) => (
                  <label
                    key={type.value}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      padding: '16px',
                      background: formData.userType === type.value 
                        ? 'rgba(0, 122, 255, 0.2)' 
                        : 'rgba(255, 255, 255, 0.05)',
                      border: formData.userType === type.value 
                        ? '1px solid rgba(0, 122, 255, 0.5)' 
                        : '1px solid rgba(255, 255, 255, 0.1)',
                      borderRadius: '12px',
                      cursor: 'pointer',
                      transition: 'all 0.3s ease'
                    }}
                    onMouseEnter={(e) => {
                      if (formData.userType !== type.value) {
                        e.currentTarget.style.background = 'rgba(255, 255, 255, 0.1)';
                        e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.2)';
                      }
                    }}
                    onMouseLeave={(e) => {
                      if (formData.userType !== type.value) {
                        e.currentTarget.style.background = 'rgba(255, 255, 255, 0.05)';
                        e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.1)';
                      }
                    }}
                  >
                    <input
                      type="radio"
                      name="userType"
                      value={type.value}
                      checked={formData.userType === type.value}
                      onChange={(e) => handleInputChange('userType', e.target.value)}
                      style={{
                        marginRight: '12px',
                        accentColor: '#007AFF'
                      }}
                    />
                    <div>
                      <div style={{
                        fontSize: '16px',
                        fontWeight: '600',
                        color: '#ffffff',
                        marginBottom: '4px'
                      }}>
                        {type.label}
                      </div>
                      <div style={{
                        fontSize: '14px',
                        color: 'rgba(255, 255, 255, 0.6)'
                      }}>
                        {type.description}
                      </div>
                    </div>
                  </label>
                ))}
              </div>
              {errors.userType && (
                <p style={{ color: '#ff6b6b', fontSize: '14px', marginTop: '8px' }}>
                  {errors.userType}
                </p>
              )}
            </div>

            {/* Research Domain */}
            <div style={{ marginBottom: '32px' }}>
              <label style={{
                display: 'block',
                fontSize: '16px',
                fontWeight: '600',
                color: '#ffffff',
                marginBottom: '12px'
              }}>
                Research Domain / Subject *
              </label>
              <select
                value={formData.domain}
                onChange={(e) => handleInputChange('domain', e.target.value)}
                style={{
                  width: '100%',
                  padding: '16px',
                  fontSize: '16px',
                  background: 'rgba(255, 255, 255, 0.05)',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  borderRadius: '12px',
                  color: '#ffffff',
                  cursor: 'pointer'
                }}
              >
                <option value="">Select your research domain</option>
                {domains.map((domain) => (
                  <option key={domain} value={domain} style={{ background: '#000000' }}>
                    {domain}
                  </option>
                ))}
              </select>
              {errors.domain && (
                <p style={{ color: '#ff6b6b', fontSize: '14px', marginTop: '8px' }}>
                  {errors.domain}
                </p>
              )}
            </div>

            {/* Project Purpose */}
            <div style={{ marginBottom: '32px' }}>
              <label style={{
                display: 'block',
                fontSize: '16px',
                fontWeight: '600',
                color: '#ffffff',
                marginBottom: '12px'
              }}>
                Project Purpose *
              </label>
              <select
                value={formData.purpose}
                onChange={(e) => handleInputChange('purpose', e.target.value)}
                style={{
                  width: '100%',
                  padding: '16px',
                  fontSize: '16px',
                  background: 'rgba(255, 255, 255, 0.05)',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  borderRadius: '12px',
                  color: '#ffffff',
                  cursor: 'pointer'
                }}
              >
                <option value="">What do you need help with?</option>
                {purposes.map((purpose) => (
                  <option key={purpose} value={purpose} style={{ background: '#000000' }}>
                    {purpose}
                  </option>
                ))}
              </select>
              {errors.purpose && (
                <p style={{ color: '#ff6b6b', fontSize: '14px', marginTop: '8px' }}>
                  {errors.purpose}
                </p>
              )}
            </div>

            {/* Contact Information */}
            <div style={{ marginBottom: '32px' }}>
              <h3 style={{
                fontSize: '18px',
                fontWeight: '600',
                color: '#ffffff',
                marginBottom: '20px'
              }}>
                Contact Information
              </h3>
              
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '20px', marginBottom: '20px' }}>
                <div>
                  <label style={{
                    display: 'block',
                    fontSize: '14px',
                    fontWeight: '500',
                    color: 'rgba(255, 255, 255, 0.8)',
                    marginBottom: '8px'
                  }}>
                    Phone Number *
                  </label>
                  <input
                    type="tel"
                    value={formData.phoneNumber}
                    onChange={(e) => handleInputChange('phoneNumber', e.target.value)}
                    placeholder="+1 (555) 123-4567"
                    style={{
                      width: '100%',
                      padding: '16px',
                      fontSize: '16px',
                      background: 'rgba(255, 255, 255, 0.05)',
                      border: '1px solid rgba(255, 255, 255, 0.1)',
                      borderRadius: '12px',
                      color: '#ffffff',
                      outline: 'none',
                      transition: 'all 0.3s ease'
                    }}
                    onFocus={(e) => {
                      e.target.style.borderColor = 'rgba(0, 122, 255, 0.5)';
                      e.target.style.background = 'rgba(255, 255, 255, 0.08)';
                    }}
                    onBlur={(e) => {
                      e.target.style.borderColor = 'rgba(255, 255, 255, 0.1)';
                      e.target.style.background = 'rgba(255, 255, 255, 0.05)';
                    }}
                  />
                  {errors.phoneNumber && (
                    <p style={{ color: '#ff6b6b', fontSize: '12px', marginTop: '4px' }}>
                      {errors.phoneNumber}
                    </p>
                  )}
                </div>

                <div>
                  <label style={{
                    display: 'block',
                    fontSize: '14px',
                    fontWeight: '500',
                    color: 'rgba(255, 255, 255, 0.8)',
                    marginBottom: '8px'
                  }}>
                    Email Address *
                  </label>
                  <input
                    type="email"
                    value={formData.email}
                    onChange={(e) => handleInputChange('email', e.target.value)}
                    placeholder="your.email@university.edu"
                    style={{
                      width: '100%',
                      padding: '16px',
                      fontSize: '16px',
                      background: 'rgba(255, 255, 255, 0.05)',
                      border: '1px solid rgba(255, 255, 255, 0.1)',
                      borderRadius: '12px',
                      color: '#ffffff',
                      outline: 'none',
                      transition: 'all 0.3s ease'
                    }}
                    onFocus={(e) => {
                      e.target.style.borderColor = 'rgba(0, 122, 255, 0.5)';
                      e.target.style.background = 'rgba(255, 255, 255, 0.08)';
                    }}
                    onBlur={(e) => {
                      e.target.style.borderColor = 'rgba(255, 255, 255, 0.1)';
                      e.target.style.background = 'rgba(255, 255, 255, 0.05)';
                    }}
                  />
                  {errors.email && (
                    <p style={{ color: '#ff6b6b', fontSize: '12px', marginTop: '4px' }}>
                      {errors.email}
                    </p>
                  )}
                </div>
              </div>
            </div>

            {/* Additional Details */}
            <div style={{ marginBottom: '32px' }}>
              <label style={{
                display: 'block',
                fontSize: '16px',
                fontWeight: '600',
                color: '#ffffff',
                marginBottom: '12px'
              }}>
                Additional Project Details
              </label>
              <textarea
                value={formData.additionalDetails}
                onChange={(e) => handleInputChange('additionalDetails', e.target.value)}
                placeholder="Describe your project requirements, timeline, budget considerations, or any specific needs..."
                rows={4}
                style={{
                  width: '100%',
                  padding: '16px',
                  fontSize: '16px',
                  background: 'rgba(255, 255, 255, 0.05)',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  borderRadius: '12px',
                  color: '#ffffff',
                  outline: 'none',
                  resize: 'vertical',
                  fontFamily: 'inherit',
                  transition: 'all 0.3s ease'
                }}
                onFocus={(e) => {
                  e.target.style.borderColor = 'rgba(0, 122, 255, 0.5)';
                  e.target.style.background = 'rgba(255, 255, 255, 0.08)';
                }}
                onBlur={(e) => {
                  e.target.style.borderColor = 'rgba(255, 255, 255, 0.1)';
                  e.target.style.background = 'rgba(255, 255, 255, 0.05)';
                }}
              />
            </div>

            {/* Submit Error */}
            {errors.submit && (
              <div style={{
                padding: '16px',
                background: 'rgba(255, 0, 0, 0.1)',
                border: '1px solid rgba(255, 0, 0, 0.3)',
                borderRadius: '12px',
                marginBottom: '20px',
                color: '#ff6b6b',
                fontSize: '14px'
              }}>
                {errors.submit}
              </div>
            )}

            {/* Submit Button */}
            <button
              type="submit"
              disabled={isSubmitting}
              style={{
                width: '100%',
                padding: '20px',
                fontSize: '18px',
                fontWeight: '600',
                background: isSubmitting 
                  ? 'rgba(255, 255, 255, 0.1)' 
                  : 'linear-gradient(135deg, #007AFF 0%, #5856D6 100%)',
                border: 'none',
                borderRadius: '16px',
                color: '#ffffff',
                cursor: isSubmitting ? 'not-allowed' : 'pointer',
                transition: 'all 0.3s ease',
                textTransform: 'none',
                letterSpacing: '-0.01em'
              }}
              onMouseEnter={(e) => {
                if (!isSubmitting) {
                  e.currentTarget.style.transform = 'translateY(-2px)';
                  e.currentTarget.style.boxShadow = '0 8px 32px rgba(0, 122, 255, 0.3)';
                }
              }}
              onMouseLeave={(e) => {
                if (!isSubmitting) {
                  e.currentTarget.style.transform = 'translateY(0)';
                  e.currentTarget.style.boxShadow = 'none';
                }
              }}
            >
              {isSubmitting ? 'Submitting Request...' : 'Submit Hire Request'}
            </button>

            {/* Professional Note */}
            <div style={{
              marginTop: '24px',
              padding: '20px',
              background: 'rgba(0, 122, 255, 0.1)',
              border: '1px solid rgba(0, 122, 255, 0.2)',
              borderRadius: '12px',
              textAlign: 'center'
            }}>
              <h4 style={{
                fontSize: '16px',
                fontWeight: '600',
                color: '#007AFF',
                marginBottom: '8px'
              }}>
                Professional Connection Process
              </h4>
              <p style={{
                fontSize: '14px',
                color: 'rgba(255, 255, 255, 0.8)',
                lineHeight: '1.5',
                margin: '0'
              }}>
                Our Gaply team will review your request, verify expert availability, 
                and facilitate a professional introduction within 24 hours. 
                You'll receive detailed project guidelines and next steps via email.
              </p>
            </div>
          </form>
        )}
      </div>
    </div>
  );
};

export default HireRequestDialog;
