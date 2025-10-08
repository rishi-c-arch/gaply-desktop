import React, { useRef, useEffect, useState } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { Environment } from '@react-three/drei';
import * as THREE from 'three';

// 3D Background Elements
const BackgroundElements: React.FC = () => {
  const groupRef = useRef<THREE.Group>(null);

  useFrame((state) => {
    if (groupRef.current) {
      groupRef.current.rotation.y = state.clock.elapsedTime * 0.1;
    }
  });

  return (
    <group ref={groupRef}>
      {/* Floating Geometric Shapes */}
      {Array.from({ length: 12 }).map((_, i) => (
        <mesh 
          key={i} 
          position={[
            Math.sin(i * 0.5) * 8,
            Math.cos(i * 0.5) * 8,
            Math.sin(i * 0.3) * 3
          ]}
        >
          <torusGeometry args={[0.3, 0.1, 8, 16]} />
          <meshStandardMaterial 
            color="#535353" 
            transparent
            opacity={0.3}
          />
        </mesh>
      ))}
      
      
      {/* Floating Particles - REMOVED */}
    </group>
  );
};

// Main 3D Free Features Component
const FreeFeatures3D: React.FC = () => {
  const containerRef = useRef<HTMLDivElement>(null);
  const [isVisible, setIsVisible] = useState(false);
  const [activeTool, setActiveTool] = useState<null | 'remover' | 'search' | 'journals'>(null);

  // Remover state
  const [removerText, setRemoverText] = useState('');
  const [removerResult, setRemoverResult] = useState<string | null>(null);

  // Search state
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<Array<{title: string; source: string; url: string}>>([]);

  // Journal matching state
  const [jmTitle, setJmTitle] = useState('');
  const [jmAbstract, setJmAbstract] = useState('');
  const [prefAccess, setPrefAccess] = useState<'free' | 'paid'>('free');
  const [prefQuartile, setPrefQuartile] = useState<'Q1' | 'Q2' | 'Q3' | 'Q4' | 'ALL'>('ALL');
  const [showImpact, setShowImpact] = useState(true);
  const [showAcceptance, setShowAcceptance] = useState(true);
  const [includeGuidelines, setIncludeGuidelines] = useState(false);
  const [journalResults, setJournalResults] = useState<Array<{
    journal: string; quartile: string; impact?: number; acceptance?: string; url: string
  }>>([]);

  useEffect(() => {
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setIsVisible(true);
        }
      },
      { threshold: 0.3 }
    );

    if (containerRef.current) {
      observer.observe(containerRef.current);
    }

    return () => observer.disconnect();
  }, []);

  return (
    <section 
      ref={containerRef}
      className="free-features-3d-section"
      style={{
        position: 'relative',
        minHeight: '100vh',
        background: 'radial-gradient(ellipse at center, #1a1a1a 0%, #2a2a2a 50%, #0A0A0A 100%)',
        overflow: 'hidden',
        display: 'flex',
        flexDirection: 'column',
        justifyContent: 'center',
        alignItems: 'center',
        padding: '80px 20px',
      }}
    >
      {/* 3D Canvas */}
      <div style={{
        position: 'absolute',
        top: 0,
        left: 0,
        width: '100%',
        height: '100%',
        zIndex: 1
      }}>
        <Canvas camera={{ position: [0, 0, 8], fov: 75 }}>
          <ambientLight intensity={0.4} />
          <directionalLight position={[5, 5, 5]} intensity={1} />
          <pointLight position={[-5, -5, -5]} intensity={0.5} />
          <spotLight position={[0, 5, 0]} intensity={0.8} angle={0.3} />
          
          <BackgroundElements />
          
          <Environment preset="night" />
        </Canvas>
      </div>

      {/* Centered Heading */}
      <div 
        className="features-heading-3d"
        style={{
          position: 'relative',
          zIndex: 2,
          textAlign: 'center',
          marginBottom: '4rem',
          opacity: isVisible ? 1 : 0,
          transform: isVisible ? 'translateY(0)' : 'translateY(50px)',
          transition: 'all 0.8s ease-out',
        }}
      >
        <h2 
          style={{
            fontSize: 'clamp(2.5rem, 6vw, 4rem)',
            fontFamily: 'Inter, sans-serif',
            fontWeight: '700',
            background: 'linear-gradient(45deg, #535353, #6c6c6c, #848484, #9d9d9d, #b5b5b5, #cecece, #e6e6e6, #ffffff)',
            backgroundSize: '400% 400%',
            WebkitBackgroundClip: 'text',
            WebkitTextFillColor: 'transparent',
            backgroundClip: 'text',
            animation: 'colorMotion 8s ease-in-out infinite',
            margin: 0,
            textShadow: '0 4px 8px rgba(0,0,0,0.3)',
            letterSpacing: '0.02em',
          }}
        >
          FREE FEATURES
        </h2>
        
        <div 
          style={{
            fontSize: 'clamp(1rem, 2.5vw, 1.4rem)',
            fontFamily: 'Inter, sans-serif',
            fontWeight: '400',
            color: '#cecece',
            marginTop: '1rem',
            opacity: 0.8,
            letterSpacing: '0.01em',
          }}
        >
          Powerful tools to accelerate your academic journey
        </div>
      </div>

      {/* Tool Cards - compact row */}
      <div
        style={{
          position: 'relative',
          zIndex: 2,
          display: 'grid',
          gridTemplateColumns: 'repeat(3, minmax(220px, 1fr))',
          gap: '1.2rem',
          width: 'min(1100px, 92vw)',
          margin: '0 auto',
        }}
      >
        {[{
          key: 'remover',
          title: '✨ Powered by AI — Academic AI Remover',
          subtitle: 'From AI text to scholarly excellence',
        }, {
          key: 'search',
          title: 'Paper Search',
          subtitle: 'Discover relevant academic papers',
        }, {
          key: 'journals',
          title: 'Journal Matching',
          subtitle: 'Find the perfect journal for your work',
        }].map((card) => (
          <button
            key={card.key}
            onClick={() => setActiveTool(card.key as typeof activeTool)}
            style={{
              perspective: '1000px',
              WebkitTapHighlightColor: 'transparent',
              border: 'none',
              background: 'transparent',
              padding: 0,
              cursor: 'pointer',
            }}
          >
            <div
              className="tool-card"
              style={{
                transformStyle: 'preserve-3d',
                transform: 'rotateX(5deg) rotateY(5deg)',
                transition: 'transform .25s ease, box-shadow .25s ease',
                background: 'linear-gradient(135deg, rgba(255,255,255,0.06), rgba(255,255,255,0.03))',
                border: '1px solid rgba(255,255,255,0.12)',
                borderRadius: '16px',
                padding: '1.25rem',
                minHeight: '140px',
                textAlign: 'left',
                color: '#e6e6e6',
                boxShadow: '0 10px 24px rgba(0,0,0,0.35)',
                backdropFilter: 'blur(8px)',
              }}
              onMouseEnter={(e) => {
                (e.currentTarget as HTMLDivElement).style.transform = 'rotateX(0deg) rotateY(0deg) scale(1.02)';
                (e.currentTarget as HTMLDivElement).style.boxShadow = '0 16px 34px rgba(0,0,0,0.45)';
              }}
              onMouseLeave={(e) => {
                (e.currentTarget as HTMLDivElement).style.transform = 'rotateX(5deg) rotateY(5deg) scale(1)';
                (e.currentTarget as HTMLDivElement).style.boxShadow = '0 10px 24px rgba(0,0,0,0.35)';
              }}
            >
              <div style={{
                fontFamily: 'Inter, sans-serif',
                fontWeight: 600,
                fontSize: '1.05rem',
                marginBottom: '.4rem',
                lineHeight: 1.25,
                background: 'linear-gradient(45deg, #e6e6e6, #b5b5b5)',
                WebkitBackgroundClip: 'text',
                WebkitTextFillColor: 'transparent',
              }}>{card.title}</div>
              <div style={{
                fontFamily: 'Inter, sans-serif',
                fontWeight: 400,
                fontSize: '.9rem',
                color: '#bdbdbd',
                opacity: .9,
              }}>{card.subtitle}</div>
              <div style={{ marginTop: '.9rem', display: 'flex', gap: '.5rem' }}>
                <span style={{
                  display: 'inline-block',
                  padding: '.35rem .6rem',
                  borderRadius: '999px',
                  background: 'rgba(255,255,255,0.08)',
                  border: '1px solid rgba(255,255,255,0.12)'
                }}>Open</span>
              </div>
            </div>
          </button>
        ))}
      </div>

      {/* Compact Modals (task windows) */}
      {activeTool && (
        <div
          role="dialog"
          aria-modal="true"
          onClick={() => setActiveTool(null)}
          style={{
            position: 'fixed', inset: 0, zIndex: 50,
            background: 'rgba(0,0,0,0.55)',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            padding: '20px'
          }}
        >
          <div
            onClick={(e) => e.stopPropagation()}
            style={{
              width: 'min(900px, 92vw)',
              background: 'linear-gradient(180deg, rgba(20,20,20,0.9), rgba(30,30,30,0.9))',
              border: '1px solid rgba(255,255,255,0.12)',
              borderRadius: '18px',
              boxShadow: '0 20px 50px rgba(0,0,0,0.5)',
              overflow: 'hidden'
            }}
          >
            <div style={{ padding: '18px 20px', borderBottom: '1px solid rgba(255,255,255,0.08)' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <h3 style={{
                  margin: 0,
                  fontFamily: 'Inter, sans-serif',
                  fontWeight: 700,
                  fontSize: '1.15rem',
                  color: '#e6e6e6'
                }}>
                  {activeTool === 'remover' && 'AI Text Remover'}
                  {activeTool === 'search' && 'Research Paper Search'}
                  {activeTool === 'journals' && 'Journal Matching'}
                </h3>
                <button onClick={() => setActiveTool(null)} style={{
                  background: 'transparent', border: 'none', color: '#cecece', cursor: 'pointer', fontSize: '1.1rem'
                }}>✕</button>
              </div>
            </div>
            <div style={{ padding: '18px 20px' }}>
              {activeTool === 'remover' && (
                <div style={{ display: 'grid', gap: '12px' }}>
                  <p style={{ margin: 0, color: '#bdbdbd' }}>From AI text to scholarly excellence — enhance your paper with precise academic language.</p>
                  <textarea rows={6} placeholder="Paste your text here for paraphrasing…" value={removerText} onChange={(e)=>setRemoverText(e.target.value)} style={{
                    width: '100%', borderRadius: '12px', padding: '12px',
                    background: 'rgba(255,255,255,0.04)', color: '#e6e6e6', border: '1px solid rgba(255,255,255,0.12)'
                  }} />
                  <div style={{ display: 'flex', justifyContent: 'center' }}>
                    <button onClick={async () => {
                      if (!removerText.trim()) return;
                      
                      try {
                        const response = await fetch('https://srv-d3cl1tmmcj7s73dmq9eg.onrender.com/api/v1/paraphrase/direct', {
                          method: 'POST',
                          headers: {
                            'Content-Type': 'application/json',
                          },
                          body: JSON.stringify({
                            text: removerText,
                          }),
                        });
                        
                        if (response.ok) {
                          const data = await response.json();
                          setRemoverResult(data.paraphrased_text || data.result || 'Paraphrasing completed');
                        } else {
                          // Fallback to simple processing if backend fails
                          setRemoverResult(removerText.replace(/\b(very|really|basically|just)\b/gi,'').trim() + ' (processed locally)');
                        }
                      } catch (error) {
                        // Fallback to simple processing if backend fails
                        setRemoverResult(removerText.replace(/\b(very|really|basically|just)\b/gi,'').trim() + ' (processed locally)');
                      }
                    }} style={{
                      padding: '10px 16px', borderRadius: '10px', border: '1px solid rgba(255,255,255,0.18)',
                      background: 'linear-gradient(135deg, #4b4b4b, #6c6c6c)', color: '#fff', cursor: 'pointer'
                    }}>Paraphrase Text</button>
                  </div>
                  {removerResult !== null && (
                    <div style={{
                      marginTop: '10px',
                      border: '1px solid rgba(255,255,255,0.12)',
                      borderRadius: '12px', padding: '12px',
                      background: 'rgba(255,255,255,0.03)', color: '#dcdcdc'
                    }}>
                      <div style={{ fontWeight: 600, marginBottom: 6 }}>Result</div>
                      <div style={{ whiteSpace: 'pre-wrap' }}>{removerResult || 'No text provided.'}</div>
                    </div>
                  )}
                </div>
              )}
              {activeTool === 'search' && (
                <div style={{ display: 'grid', gap: '12px' }}>
                  <p style={{ margin: 0, color: '#bdbdbd' }}>Discover relevant academic papers with AI‑powered search.</p>
                  <input value={searchQuery} onChange={(e)=>setSearchQuery(e.target.value)} placeholder="Enter your research query (e.g., 'renewable energy solutions')" style={{
                    width: '100%', borderRadius: '12px', padding: '12px',
                    background: 'rgba(255,255,255,0.04)', color: '#e6e6e6', border: '1px solid rgba(255,255,255,0.12)'
                  }} />
                  <div style={{ display: 'flex', justifyContent: 'center' }}>
                    <button onClick={async () => {
                      if (!searchQuery.trim()) return;
                      
                      try {
                        const response = await fetch('https://srv-d3cl1tmmcj7s73dmq9eg.onrender.com/api/search', {
                          method: 'POST',
                          headers: {
                            'Content-Type': 'application/json',
                          },
                          body: JSON.stringify({
                            query: searchQuery,
                          }),
                        });
                        
                        if (response.ok) {
                          const data = await response.json();
                          // Handle different response formats
                          if (data.papers && Array.isArray(data.papers)) {
                            setSearchResults(data.papers.map((paper: any, i: number) => ({
                              title: paper.title || `${searchQuery} — Study ${i+1}`,
                              source: paper.source || paper.publisher || ['arXiv','CrossRef','OpenAlex'][i%3],
                              url: paper.url || paper.link || '#'
                            })));
                          } else if (data.results && Array.isArray(data.results)) {
                            setSearchResults(data.results.map((result: any, i: number) => ({
                              title: result.title || `${searchQuery} — Study ${i+1}`,
                              source: result.source || ['arXiv','CrossRef','OpenAlex'][i%3],
                              url: result.url || '#'
                            })));
                          } else {
                            // Fallback to mock data if backend format is unexpected
                            setSearchResults(Array.from({length:3}).map((_,i)=>({
                              title: `${searchQuery} — Study ${i+1}`,
                              source: ['arXiv','CrossRef','OpenAlex'][i%3],
                              url: '#'
                            })));
                          }
                        } else {
                          // Fallback to mock data if backend fails
                          setSearchResults(Array.from({length:3}).map((_,i)=>({
                            title: `${searchQuery} — Study ${i+1}`,
                            source: ['arXiv','CrossRef','OpenAlex'][i%3],
                            url: '#'
                          })));
                        }
                      } catch (error) {
                        // Fallback to mock data if backend fails
                        setSearchResults(Array.from({length:3}).map((_,i)=>({
                          title: `${searchQuery} — Study ${i+1}`,
                          source: ['arXiv','CrossRef','OpenAlex'][i%3],
                          url: '#'
                        })));
                      }
                    }} style={{
                      padding: '10px 16px', borderRadius: '10px', border: '1px solid rgba(255,255,255,0.18)',
                      background: 'linear-gradient(135deg, #4b4b4b, #6c6c6c)', color: '#fff', cursor: 'pointer'
                    }}>Search Papers</button>
                  </div>
                  {searchResults.length > 0 && (
                    <div style={{
                      marginTop: '10px',
                      border: '1px solid rgba(255,255,255,0.12)',
                      borderRadius: '12px', padding: '10px',
                      background: 'rgba(255,255,255,0.03)'
                    }}>
                      <div style={{ fontWeight: 600, color:'#dcdcdc', marginBottom: 6 }}>Results</div>
                      <ul style={{ margin:0, paddingLeft:'18px', color:'#cfcfcf' }}>
                        {searchResults.map((r, idx)=> (
                          <li key={idx} style={{ marginBottom: 6 }}>
                            <a href={r.url} style={{ color:'#e6e6e6', textDecoration:'none' }}>{r.title}</a>
                            <span style={{ opacity:.7 }}> · {r.source}</span>
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}
                </div>
              )}
              {activeTool === 'journals' && (
                <div style={{ display: 'grid', gap: '12px' }}>
                  <p style={{ margin: 0, color: '#bdbdbd' }}>Find the perfect journal for your research. Get personalized recommendations.</p>

                  {/* Preferences row */}
                  <div style={{
                    display:'flex', flexWrap:'wrap', gap:'10px',
                    border:'1px solid rgba(255,255,255,0.12)', borderRadius:'12px', padding:'10px',
                    background:'rgba(255,255,255,0.03)'
                  }}>
                    <div style={{ color:'#dcdcdc', fontWeight:600 }}>Select Your Preferences:</div>
                    <label style={{ display:'flex', alignItems:'center', gap:6, color:'#cfcfcf' }}>
                      <input type="radio" checked={prefAccess==='free'} onChange={()=>setPrefAccess('free')} /> Free Access Only
                    </label>
                    <label style={{ display:'flex', alignItems:'center', gap:6, color:'#cfcfcf' }}>
                      <input type="radio" checked={prefAccess==='paid'} onChange={()=>setPrefAccess('paid')} /> Paid Access OK
                    </label>
                    <div style={{ width:'100%', height:0 }} />
                    {(['Q1','Q2','Q3','Q4','ALL'] as const).map(q=> (
                      <label key={q} style={{ display:'flex', alignItems:'center', gap:6, color:'#cfcfcf' }}>
                        <input type="radio" checked={prefQuartile===q} onChange={()=>setPrefQuartile(q)} /> {q==='ALL' ? 'All Quartiles' : q+ ' (tier)'}
                      </label>
                    ))}
                  </div>

                  <input value={jmTitle} onChange={(e)=>setJmTitle(e.target.value)} placeholder="Enter your research title…" style={{
                    width: '100%', borderRadius: '12px', padding: '12px',
                    background: 'rgba(255,255,255,0.04)', color: '#e6e6e6', border: '1px solid rgba(255,255,255,0.12)'
                  }} />
                  <textarea rows={4} value={jmAbstract} onChange={(e)=>setJmAbstract(e.target.value)} placeholder="Paste your abstract here… (Minimum 100 words recommended)" style={{
                    width: '100%', borderRadius: '12px', padding: '12px',
                    background: 'rgba(255,255,255,0.04)', color: '#e6e6e6', border: '1px solid rgba(255,255,255,0.12)'
                  }} />
                  <div style={{ display:'flex', gap:'14px', flexWrap:'wrap', color:'#cfcfcf' }}>
                    <label style={{ display:'flex', alignItems:'center', gap:6 }}>
                      <input type="checkbox" checked={showImpact} onChange={(e)=>setShowImpact(e.target.checked)} /> Show Impact Factor
                    </label>
                    <label style={{ display:'flex', alignItems:'center', gap:6 }}>
                      <input type="checkbox" checked={showAcceptance} onChange={(e)=>setShowAcceptance(e.target.checked)} /> Show Acceptance Rate
                    </label>
                    <label style={{ display:'flex', alignItems:'center', gap:6 }}>
                      <input type="checkbox" checked={includeGuidelines} onChange={(e)=>setIncludeGuidelines(e.target.checked)} /> Include Submission Guidelines
                    </label>
                  </div>
                  <div style={{ display: 'flex', justifyContent: 'center' }}>
                    <button onClick={async () => {
                      if (!jmTitle.trim() || !jmAbstract.trim()) {
                        // Fallback to mock data if no input
                        const base = [
                          { journal:'IEEE Access', quartile:'Q1', impact:4.64, acceptance:'30%', url:'#' },
                          { journal:'PLOS ONE', quartile:'Q2', impact:3.75, acceptance:'48%', url:'#' },
                          { journal:'Heliyon', quartile:'Q3', impact:3.2, acceptance:'40%', url:'#' },
                        ];
                        const filtered = prefQuartile==='ALL' ? base : base.filter(b=>b.quartile===prefQuartile);
                        setJournalResults(filtered.map(b=> ({
                          journal: b.journal, quartile:b.quartile,
                          impact: showImpact ? b.impact : undefined,
                          acceptance: showAcceptance ? b.acceptance : undefined,
                          url: b.url
                        })));
                        return;
                      }
                      
                      try {
                        const response = await fetch('https://srv-d3cl1tmmcj7s73dmq9eg.onrender.com/api/match', {
                          method: 'POST',
                          headers: {
                            'Content-Type': 'application/json',
                          },
                          body: JSON.stringify({
                            title: jmTitle,
                            abstract: jmAbstract,
                            preferences: {
                              access: prefAccess,
                              quartile: prefQuartile,
                              showImpact,
                              showAcceptance,
                              includeGuidelines
                            }
                          }),
                        });
                        
                        if (response.ok) {
                          const data = await response.json();
                          // Handle different response formats
                          if (data.journals && Array.isArray(data.journals)) {
                            setJournalResults(data.journals.map((journal: any) => ({
                              journal: journal.name || journal.journal,
                              quartile: journal.quartile || journal.tier || 'Q2',
                              impact: showImpact ? (journal.impact_factor || journal.impact) : undefined,
                              acceptance: showAcceptance ? (journal.acceptance_rate || journal.acceptance) : undefined,
                              url: journal.url || journal.link || '#'
                            })));
                          } else if (data.results && Array.isArray(data.results)) {
                            setJournalResults(data.results.map((result: any) => ({
                              journal: result.journal || result.name,
                              quartile: result.quartile || result.tier || 'Q2',
                              impact: showImpact ? result.impact : undefined,
                              acceptance: showAcceptance ? result.acceptance : undefined,
                              url: result.url || '#'
                            })));
                          } else {
                            // Fallback to mock data if backend format is unexpected
                            const base = [
                              { journal:'IEEE Access', quartile:'Q1', impact:4.64, acceptance:'30%', url:'#' },
                              { journal:'PLOS ONE', quartile:'Q2', impact:3.75, acceptance:'48%', url:'#' },
                            ];
                            setJournalResults(base.map(b=> ({
                              journal: b.journal, quartile:b.quartile,
                              impact: showImpact ? b.impact : undefined,
                              acceptance: showAcceptance ? b.acceptance : undefined,
                              url: b.url
                            })));
                          }
                        } else {
                          // Fallback to mock data if backend fails
                          const base = [
                            { journal:'IEEE Access', quartile:'Q1', impact:4.64, acceptance:'30%', url:'#' },
                            { journal:'PLOS ONE', quartile:'Q2', impact:3.75, acceptance:'48%', url:'#' },
                          ];
                          setJournalResults(base.map(b=> ({
                            journal: b.journal, quartile:b.quartile,
                            impact: showImpact ? b.impact : undefined,
                            acceptance: showAcceptance ? b.acceptance : undefined,
                            url: b.url
                          })));
                        }
                      } catch (error) {
                        // Fallback to mock data if backend fails
                        const base = [
                          { journal:'IEEE Access', quartile:'Q1', impact:4.64, acceptance:'30%', url:'#' },
                          { journal:'PLOS ONE', quartile:'Q2', impact:3.75, acceptance:'48%', url:'#' },
                        ];
                        setJournalResults(base.map(b=> ({
                          journal: b.journal, quartile:b.quartile,
                          impact: showImpact ? b.impact : undefined,
                          acceptance: showAcceptance ? b.acceptance : undefined,
                          url: b.url
                        })));
                      }
                    }} style={{
                      padding: '10px 16px', borderRadius: '10px', border: '1px solid rgba(255,255,255,0.18)',
                      background: 'linear-gradient(135deg, #4b4b4b, #6c6c6c)', color: '#fff', cursor: 'pointer'
                    }}>Find Matching Journals</button>
                  </div>
                  {journalResults.length > 0 && (
                    <div style={{
                      marginTop: '10px',
                      border: '1px solid rgba(255,255,255,0.12)',
                      borderRadius: '12px', padding: '10px',
                      background: 'rgba(255,255,255,0.03)'
                    }}>
                      <div style={{ fontWeight: 600, color:'#dcdcdc', marginBottom: 6 }}>Results</div>
                      <ul style={{ margin:0, paddingLeft:'18px', color:'#cfcfcf' }}>
                        {journalResults.map((r, idx)=> (
                          <li key={idx} style={{ marginBottom: 8 }}>
                            <a href={r.url} style={{ color:'#e6e6e6', textDecoration:'none' }}>{r.journal}</a>
                            <span style={{ opacity:.7 }}> · {r.quartile}</span>
                            {r.impact !== undefined && <span style={{ opacity:.7 }}> · IF {r.impact}</span>}
                            {r.acceptance && <span style={{ opacity:.7 }}> · Acceptance {r.acceptance}</span>}
                            {includeGuidelines && <span style={{ opacity:.7 }}> · Guidelines included</span>}
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {/* Floating Action Button */}
      <div 
        style={{
          position: 'absolute',
          bottom: '2rem',
          right: '2rem',
          zIndex: 3,
          opacity: isVisible ? 1 : 0,
          transform: isVisible ? 'translateY(0)' : 'translateY(20px)',
          transition: 'all 0.8s ease-out 0.4s',
        }}
      >
        <button
          style={{
            width: '60px',
            height: '60px',
            borderRadius: '50%',
            background: 'linear-gradient(45deg, #535353, #6c6c6c, #848484)',
            border: 'none',
            color: '#ffffff',
            fontSize: '1.5rem',
            cursor: 'pointer',
            boxShadow: '0 10px 20px rgba(0,0,0,0.3)',
            transition: 'all 0.3s ease-out',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.transform = 'scale(1.1) rotate(180deg)';
            e.currentTarget.style.boxShadow = '0 15px 30px rgba(0,0,0,0.4)';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.transform = 'scale(1) rotate(0deg)';
            e.currentTarget.style.boxShadow = '0 10px 20px rgba(0,0,0,0.3)';
          }}
        >
          ↑
        </button>
      </div>
    </section>
  );
};

export default FreeFeatures3D;
