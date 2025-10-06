import React, { useState, useEffect } from 'react';
import { buildApiUrl } from '../api/config';

interface PremiumFeatureProps {
  featureType: 'gapFinder' | 'deepEvaluation';
  onClose: () => void;
}

const PremiumFeatureModal: React.FC<PremiumFeatureProps> = ({ featureType, onClose }) => {
  const [userPlan, setUserPlan] = useState<any>(null);
  const [loading, setLoading] = useState(true);
  const [processing, setProcessing] = useState(false);
  const [result, setResult] = useState<string | null>(null);
  const [uploadedFiles, setUploadedFiles] = useState<File[]>([]);

  useEffect(() => {
    checkUserPlan();
  }, []);

  const checkUserPlan = async () => {
    const token = localStorage.getItem('gaply_token');
    if (!token) {
      alert('Please login first');
      onClose();
      return;
    }

    try {
      const response = await fetch(buildApiUrl('/api/v1/user/account'), {
        headers: {
          'Authorization': `Bearer ${token}`,
          'Content-Type': 'application/json'
        }
      });

      if (response.ok) {
        const data = await response.json();
        setUserPlan(data);
      }
    } catch (error) {
      console.error('Failed to fetch user plan:', error);
    } finally {
      setLoading(false);
    }
  };

  const handleFileUpload = (event: React.ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(event.target.files || []);
    setUploadedFiles(files);
  };

  const processFeature = async () => {
    if (!userPlan) return;

    const hasUses = featureType === 'gapFinder' 
      ? userPlan.gapFinderUsesRemaining > 0 
      : userPlan.deepEvalUsesRemaining > 0;

    if (!hasUses) {
      alert('No uses remaining. Please upgrade your plan.');
      return;
    }

    if (uploadedFiles.length === 0) {
      alert('Please upload at least one file');
      return;
    }

    setProcessing(true);

    try {
      const formData = new FormData();
      uploadedFiles.forEach((file, index) => {
        formData.append(`file_${index}`, file);
      });

      const endpoint = featureType === 'gapFinder' 
        ? '/api/premium-features/literature-analysis'
        : '/api/premium-features/deep-paper-analysis';

      const response = await fetch(buildApiUrl(endpoint), {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${localStorage.getItem('gaply_token')}`
        },
        body: formData
      });

      const data = await response.json();

      if (data.success) {
        setResult(data.data.report_url || data.data.analysis);
        
        // Update usage count
        await updateUsage();
      } else {
        alert('Processing failed: ' + data.error);
      }
    } catch (error) {
      console.error('Feature processing failed:', error);
      alert('Processing failed. Please try again.');
    } finally {
      setProcessing(false);
    }
  };

  const updateUsage = async () => {
    const token = localStorage.getItem('gaply_token');
    if (!token) return;

    try {
      const newUses = featureType === 'gapFinder' 
        ? userPlan.gapFinderUsesRemaining - 1
        : userPlan.deepEvalUsesRemaining - 1;

      await fetch(buildApiUrl('/api/v1/user/usage'), {
        method: 'PUT',
        headers: {
          'Authorization': `Bearer ${token}`,
          'Content-Type': 'application/json'
        },
        body: JSON.stringify({
          feature: featureType,
          uses_remaining: newUses
        })
      });

      // Update local state
      setUserPlan((prev: any) => prev ? {
        ...prev,
        [featureType === 'gapFinder' ? 'gapFinderUsesRemaining' : 'deepEvalUsesRemaining']: newUses
      } : null);
    } catch (error) {
      console.error('Failed to update usage:', error);
    }
  };

  const downloadReport = () => {
    if (result) {
      window.open(result, '_blank');
    }
  };

  if (loading) {
    return (
      <div style={{
        position: 'fixed',
        top: 0,
        left: 0,
        width: '100%',
        height: '100%',
        background: 'rgba(0, 0, 0, 0.8)',
        display: 'flex',
        justifyContent: 'center',
        alignItems: 'center',
        zIndex: 10000
      }}>
        <div style={{ color: '#cecece', fontSize: '1.2rem' }}>Loading...</div>
      </div>
    );
  }

  if (!userPlan) {
    return (
      <div style={{
        position: 'fixed',
        top: 0,
        left: 0,
        width: '100%',
        height: '100%',
        background: 'rgba(0, 0, 0, 0.8)',
        display: 'flex',
        justifyContent: 'center',
        alignItems: 'center',
        zIndex: 10000
      }}>
        <div style={{
          background: '#1a1a1a',
          padding: '40px',
          borderRadius: '20px',
          textAlign: 'center',
          maxWidth: '500px',
          color: '#cecece'
        }}>
          <h3 style={{ color: '#ff7a1a', marginBottom: '20px' }}>Premium Access Required</h3>
          <p style={{ marginBottom: '30px' }}>
            You need a premium plan to access this feature.
          </p>
          <button
            onClick={() => window.location.href = '/premium'}
            style={{
              background: 'linear-gradient(45deg, #ff7a1a, #ff9500)',
              color: 'white',
              border: 'none',
              padding: '12px 24px',
              borderRadius: '20px',
              fontSize: '1rem',
              fontWeight: '600',
              cursor: 'pointer',
              marginRight: '10px'
            }}
          >
            View Plans
          </button>
          <button
            onClick={onClose}
            style={{
              background: 'transparent',
              color: '#cecece',
              border: '1px solid #cecece',
              padding: '12px 24px',
              borderRadius: '20px',
              fontSize: '1rem',
              cursor: 'pointer'
            }}
          >
            Close
          </button>
        </div>
      </div>
    );
  }

  const hasUses = featureType === 'gapFinder' 
    ? userPlan.gapFinderUsesRemaining > 0 
    : userPlan.deepEvalUsesRemaining > 0;

  const usesRemaining = featureType === 'gapFinder' 
    ? userPlan.gapFinderUsesRemaining 
    : userPlan.deepEvalUsesRemaining;

  return (
    <div style={{
      position: 'fixed',
      top: 0,
      left: 0,
      width: '100%',
      height: '100%',
      background: 'rgba(0, 0, 0, 0.8)',
      display: 'flex',
      justifyContent: 'center',
      alignItems: 'center',
      zIndex: 10000
    }}>
      <div style={{
        background: '#1a1a1a',
        padding: '40px',
        borderRadius: '20px',
        maxWidth: '600px',
        width: '90%',
        maxHeight: '90vh',
        overflowY: 'auto',
        color: '#cecece'
      }}>
        {/* Header */}
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '30px' }}>
          <h2 style={{ 
            color: '#ff7a1a', 
            margin: 0,
            fontSize: '1.8rem'
          }}>
            {featureType === 'gapFinder' ? '🔍 Advanced Literature Analysis' : '📊 Deep Paper Analysis'}
          </h2>
          <button
            onClick={onClose}
            style={{
              background: 'transparent',
              border: 'none',
              color: '#cecece',
              fontSize: '1.5rem',
              cursor: 'pointer'
            }}
          >
            ×
          </button>
        </div>

        {/* Usage Status */}
        <div style={{
          background: 'rgba(255, 122, 26, 0.1)',
          border: '1px solid rgba(255, 122, 26, 0.3)',
          borderRadius: '10px',
          padding: '15px',
          marginBottom: '30px',
          textAlign: 'center'
        }}>
          <strong>Uses Remaining: {usesRemaining}</strong>
          {!hasUses && (
            <p style={{ color: '#ff7a1a', margin: '10px 0 0 0' }}>
              No uses remaining. <a href="/premium" style={{ color: '#ff7a1a' }}>Upgrade your plan</a>
            </p>
          )}
        </div>

        {/* File Upload */}
        <div style={{ marginBottom: '30px' }}>
          <h3 style={{ marginBottom: '15px' }}>Upload Research Papers</h3>
          <input
            type="file"
            multiple
            accept=".pdf,.doc,.docx"
            onChange={handleFileUpload}
            style={{
              width: '100%',
              padding: '10px',
              border: '1px solid #444',
              borderRadius: '8px',
              background: '#2a2a2a',
              color: '#cecece',
              marginBottom: '15px'
            }}
          />
          {uploadedFiles.length > 0 && (
            <div>
              <p style={{ marginBottom: '10px' }}>Selected files:</p>
              {uploadedFiles.map((file, index) => (
                <div key={index} style={{ 
                  background: '#2a2a2a', 
                  padding: '8px', 
                  borderRadius: '5px', 
                  marginBottom: '5px',
                  fontSize: '0.9rem'
                }}>
                  {file.name}
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Feature Description */}
        <div style={{ marginBottom: '30px' }}>
          <h3 style={{ marginBottom: '15px' }}>What this feature does:</h3>
          <div style={{ background: '#2a2a2a', padding: '15px', borderRadius: '8px' }}>
            {featureType === 'gapFinder' ? (
              <ul style={{ margin: 0, paddingLeft: '20px' }}>
                <li>Analyzes up to 5 research papers</li>
                <li>Identifies research gaps and opportunities</li>
                <li>Generates professional analysis report</li>
                <li>Provides actionable recommendations</li>
              </ul>
            ) : (
              <ul style={{ margin: 0, paddingLeft: '20px' }}>
                <li>70-parameter comprehensive evaluation</li>
                <li>Journal compliance checking (Q1-Q4)</li>
                <li>Evidence-based analysis</li>
                <li>Professional HTML report generation</li>
              </ul>
            )}
          </div>
        </div>

        {/* Action Buttons */}
        <div style={{ display: 'flex', gap: '15px', justifyContent: 'center' }}>
          <button
            onClick={processFeature}
            disabled={!hasUses || uploadedFiles.length === 0 || processing}
            style={{
              background: processing || !hasUses || uploadedFiles.length === 0
                ? 'rgba(255, 122, 26, 0.5)'
                : 'linear-gradient(45deg, #ff7a1a, #ff9500)',
              color: 'white',
              border: 'none',
              padding: '15px 30px',
              borderRadius: '25px',
              fontSize: '1rem',
              fontWeight: '600',
              cursor: processing || !hasUses || uploadedFiles.length === 0 ? 'not-allowed' : 'pointer',
              transition: 'all 0.3s ease'
            }}
          >
            {processing ? 'Processing...' : 'Start Analysis'}
          </button>

          {result && (
            <button
              onClick={downloadReport}
              style={{
                background: 'transparent',
                color: '#ff7a1a',
                border: '1px solid #ff7a1a',
                padding: '15px 30px',
                borderRadius: '25px',
                fontSize: '1rem',
                fontWeight: '600',
                cursor: 'pointer',
                transition: 'all 0.3s ease'
              }}
            >
              Download Report
            </button>
          )}
        </div>

        {/* Result Display */}
        {result && (
          <div style={{
            marginTop: '30px',
            background: '#2a2a2a',
            padding: '20px',
            borderRadius: '10px',
            textAlign: 'center'
          }}>
            <h4 style={{ color: '#ff7a1a', marginBottom: '15px' }}>Analysis Complete!</h4>
            <p>Your report has been generated successfully.</p>
            <button
              onClick={downloadReport}
              style={{
                background: 'linear-gradient(45deg, #ff7a1a, #ff9500)',
                color: 'white',
                border: 'none',
                padding: '10px 20px',
                borderRadius: '20px',
                fontSize: '0.9rem',
                fontWeight: '600',
                cursor: 'pointer',
                marginTop: '10px'
              }}
            >
              Open Report
            </button>
          </div>
        )}
      </div>
    </div>
  );
};

export default PremiumFeatureModal;
