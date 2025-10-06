import React, { useState, useRef } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { Environment } from '@react-three/drei';
import * as THREE from 'three';
import './PremiumFeatures.css';

// 3D Background for Feature Pages
const FeatureBackground: React.FC = () => {
  const groupRef = useRef<THREE.Group>(null);

  useFrame((state) => {
    if (groupRef.current) {
      groupRef.current.rotation.y = state.clock.elapsedTime * 0.03;
    }
  });

  return (
    <group ref={groupRef}>
      {/* Floating Research Elements */}
      {Array.from({ length: 6 }).map((_, i) => (
        <mesh 
          key={i} 
          position={[
            Math.sin(i * 1.0) * 10,
            Math.cos(i * 1.0) * 10,
            Math.sin(i * 0.6) * 3
          ]}
        >
          <boxGeometry args={[0.3, 0.3, 0.3]} />
          <meshStandardMaterial 
            color="#ff7a1a" 
            transparent
            opacity={0.2}
            metalness={0.7}
            roughness={0.3}
          />
        </mesh>
      ))}
    </group>
  );
};

interface PremiumFeaturePageProps {
  featureType: 'gap-finder' | 'deep-analysis';
}

const PremiumFeaturePage: React.FC<PremiumFeaturePageProps> = ({ featureType }) => {
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [userPlan, setUserPlan] = useState<any>(null);
  const [loading, setLoading] = useState(true);
  const [uploadedFiles, setUploadedFiles] = useState<File[]>([]);
  const [analysisResult, setAnalysisResult] = useState<any>(null);
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const featureConfig = {
    'gap-finder': {
      title: 'Research Gap Finder',
      description: 'Analyze multiple research papers to identify unexplored areas and research opportunities',
      icon: '🔍',
      maxFiles: 5,
      endpoint: '/api/premium-features/literature-analysis',
      features: [
        'Upload up to 5 research papers',
        'AI-powered gap identification',
        'Professional report generation',
        'Publishable problem statements',
        'Clear objectives & hypotheses'
      ]
    },
    'deep-analysis': {
      title: 'Deep Paper Analysis',
      description: 'Comprehensive evaluation with 70+ parameters for journal compliance and quality assessment',
      icon: '📊',
      maxFiles: 1,
      endpoint: '/api/premium-features/deep-paper-analysis',
      features: [
        '70+ parameter evaluation',
        'Journal compliance checking',
        'Evidence-based analysis',
        'Professional HTML reports',
        'Q1-Q4 journal recommendations'
      ]
    }
  };

  const config = featureConfig[featureType];

  React.useEffect(() => {
    checkAuthentication();
  }, []);

  const checkAuthentication = async () => {
    const token = localStorage.getItem('gaply_token');
    if (token) {
      try {
        const response = await fetch('https://backend.gaply.in/api/v1/user/profile', {
          headers: {
            'Authorization': `Bearer ${token}`,
            'Content-Type': 'application/json'
          }
        });
        
        if (response.ok) {
          const userData = await response.json();
          setIsAuthenticated(true);
          setUserPlan(userData.plan);
        }
      } catch (error) {
        console.error('Auth check failed:', error);
      }
    }
    setLoading(false);
  };

  const handleFileUpload = (event: React.ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(event.target.files || []);
    const validFiles = files.filter(file => file.type === 'application/pdf');
    
    if (validFiles.length + uploadedFiles.length > config.maxFiles) {
      alert(`You can only upload up to ${config.maxFiles} files`);
      return;
    }
    
    setUploadedFiles(prev => [...prev, ...validFiles]);
  };

  const removeFile = (index: number) => {
    setUploadedFiles(prev => prev.filter((_, i) => i !== index));
  };

  const handleAnalysis = async () => {
    if (uploadedFiles.length === 0) {
      alert('Please upload at least one file');
      return;
    }

    if (!isAuthenticated) {
      alert('Please login to use premium features');
      return;
    }

    setIsAnalyzing(true);

    try {
      const formData = new FormData();
      uploadedFiles.forEach((file, index) => {
        formData.append(`file_${index}`, file);
      });

      const token = localStorage.getItem('gaply_token');
      const response = await fetch(`https://backend.gaply.in${config.endpoint}`, {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${token}`,
        },
        body: formData
      });

      if (response.ok) {
        const result = await response.json();
        setAnalysisResult(result);
      } else {
        const error = await response.json();
        alert(error.message || 'Analysis failed');
      }
    } catch (error) {
      console.error('Analysis error:', error);
      alert('Analysis failed. Please try again.');
    } finally {
      setIsAnalyzing(false);
    }
  };

  const downloadReport = () => {
    if (analysisResult?.report_url) {
      window.open(analysisResult.report_url, '_blank');
    }
  };

  if (loading) {
    return (
      <div className="premium-loading">
        <div className="loading-spinner"></div>
        <p>Loading premium feature...</p>
      </div>
    );
  }

  if (!isAuthenticated) {
    return (
      <div className="premium-auth-required">
        <div className="auth-required-content">
          <h2>Authentication Required</h2>
          <p>Please login to access premium features</p>
          <a href="/login" className="auth-btn">Login</a>
        </div>
      </div>
    );
  }

  return (
    <div className="premium-feature-page">
      {/* 3D Background */}
      <div className="feature-background-3d">
        <Canvas camera={{ position: [0, 0, 8], fov: 75 }}>
          <ambientLight intensity={0.3} />
          <directionalLight position={[10, 10, 5]} intensity={1} />
          <pointLight position={[-10, -10, -5]} intensity={0.5} color="#ff7a1a" />
          <Environment preset="night" />
          <FeatureBackground />
        </Canvas>
      </div>

      {/* Header */}
      <div className="feature-header">
        <div className="feature-nav">
          <a href="/" className="feature-logo">Gaply</a>
          <div className="feature-nav-links">
            <a href="/premium">Premium</a>
            <a href="/dashboard">Dashboard</a>
            <a href="/logout" className="logout-btn">Logout</a>
          </div>
        </div>
      </div>

      {/* Main Content */}
      <div className="feature-main">
        <div className="container">
          {/* Feature Header */}
          <div className="feature-hero">
            <div className="feature-icon-large">{config.icon}</div>
            <h1 className="feature-title">{config.title}</h1>
            <p className="feature-description">{config.description}</p>
            
            {userPlan && (
              <div className="plan-info">
                <span>Current Plan: {userPlan.name}</span>
                <span>Uses Remaining: {userPlan[featureType === 'gap-finder' ? 'gapFinderUses' : 'deepAnalysisUses']}</span>
              </div>
            )}
          </div>

          {/* Upload Section */}
          <div className="upload-section">
            <h3>Upload Research Papers</h3>
            <div className="upload-area" onClick={() => fileInputRef.current?.click()}>
              <div className="upload-icon">📄</div>
              <p>Click to upload PDF files</p>
              <p className="upload-limit">Maximum {config.maxFiles} files</p>
            </div>
            
            <input
              ref={fileInputRef}
              type="file"
              multiple
              accept=".pdf"
              onChange={handleFileUpload}
              style={{ display: 'none' }}
            />

            {/* Uploaded Files */}
            {uploadedFiles.length > 0 && (
              <div className="uploaded-files">
                <h4>Uploaded Files:</h4>
                {uploadedFiles.map((file, index) => (
                  <div key={index} className="file-item">
                    <span className="file-name">{file.name}</span>
                    <button 
                      className="remove-file-btn"
                      onClick={() => removeFile(index)}
                    >
                      ×
                    </button>
                  </div>
                ))}
              </div>
            )}

            {/* Analysis Button */}
            <button 
              className="analyze-btn"
              onClick={handleAnalysis}
              disabled={uploadedFiles.length === 0 || isAnalyzing}
            >
              {isAnalyzing ? 'Analyzing...' : `Start ${config.title}`}
            </button>
          </div>

          {/* Results Section */}
          {analysisResult && (
            <div className="results-section">
              <h3>Analysis Results</h3>
              <div className="results-content">
                <div className="results-summary">
                  <h4>Summary</h4>
                  <p>{analysisResult.summary}</p>
                </div>
                
                {analysisResult.gaps && (
                  <div className="gaps-section">
                    <h4>Research Gaps Identified</h4>
                    <ul>
                      {analysisResult.gaps.map((gap: string, index: number) => (
                        <li key={index}>{gap}</li>
                      ))}
                    </ul>
                  </div>
                )}

                {analysisResult.recommendations && (
                  <div className="recommendations-section">
                    <h4>Recommendations</h4>
                    <ul>
                      {analysisResult.recommendations.map((rec: string, index: number) => (
                        <li key={index}>{rec}</li>
                      ))}
                    </ul>
                  </div>
                )}

                <button 
                  className="download-report-btn"
                  onClick={downloadReport}
                >
                  Download Full Report
                </button>
              </div>
            </div>
          )}

          {/* Features List */}
          <div className="features-list">
            <h3>What you get:</h3>
            <ul>
              {config.features.map((feature, index) => (
                <li key={index}>
                  <span className="checkmark">✓</span>
                  {feature}
                </li>
              ))}
            </ul>
          </div>
        </div>
      </div>
    </div>
  );
};

export default PremiumFeaturePage;
