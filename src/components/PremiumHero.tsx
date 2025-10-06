import React, { useState, useRef } from 'react';

interface PremiumHeroProps {
  onGetStarted?: () => void;
  onViewDocs?: () => void;
}

export default function PremiumHero({ onGetStarted, onViewDocs }: PremiumHeroProps) {
  const [codeExpanded, setCodeExpanded] = useState(false);
  const [copied, setCopied] = useState(false);
  const codeRef = useRef<HTMLPreElement>(null);

  const codeSnippet = `import { gaply } from '@gaply/sdk';

const response = await gaply.search({
  query: "machine learning research",
  filters: { year: 2024, type: "journal" },
  limit: 50
});

console.log(response.papers);`;

  const handleCopyCode = async () => {
    if (codeRef.current) {
      try {
        await navigator.clipboard.writeText(codeSnippet);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
      } catch (err) {
        console.error('Failed to copy code:', err);
      }
    }
  };

  return (
    <section className="bg-white relative overflow-hidden">
      {/* Subtle background pattern */}
      <div className="absolute inset-0 opacity-5">
        <div className="absolute top-20 left-10 w-32 h-32 bg-primary rounded-full blur-3xl"></div>
        <div className="absolute bottom-20 right-10 w-40 h-40 bg-primary rounded-full blur-3xl"></div>
        <div className="absolute top-1/2 left-1/3 w-24 h-24 bg-primary rounded-full blur-2xl"></div>
      </div>

      <div className="max-w-6xl mx-auto px-6 py-20 lg:py-28 relative">
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-16 items-center">
          {/* Left Content */}
          <div className="space-y-8">
            {/* Main Headline */}
            <div className="space-y-6">
              <h1 className="text-4xl md:text-5xl lg:text-6xl font-medium text-neutral-900 leading-tight tracking-tight">
                Research that makes{' '}
                <span className="text-primary font-semibold">genius</span> look effortless
              </h1>
              
              <p className="text-xl text-neutral-700 max-w-2xl leading-relaxed">
                Discover relevant papers, get AI-powered insights, and accelerate your research with our developer-first platform.
              </p>
            </div>

            {/* CTAs */}
            <div className="flex flex-col sm:flex-row items-start gap-4">
              <button 
                onClick={onGetStarted}
                className="inline-flex items-center px-8 py-4 bg-primary text-white rounded-lg text-base font-semibold shadow-soft hover:bg-primary-600 hover:shadow-pop transition-all duration-200 hover:scale-105"
              >
                Start Researching
                <svg className="ml-2 w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 7l5 5m0 0l-5 5m5-5H6" />
                </svg>
              </button>
              
              <button 
                onClick={onViewDocs}
                className="inline-flex items-center px-6 py-4 border border-neutral-300 rounded-lg text-base text-neutral-700 hover:bg-neutral-50 hover:border-neutral-400 transition-all duration-200"
              >
                View Documentation
                <svg className="ml-2 w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
                </svg>
              </button>
            </div>

            {/* Stats */}
            <div className="grid grid-cols-3 gap-8 py-8 border-t border-neutral-200">
              <div className="text-center">
                <div className="text-3xl font-bold text-neutral-900">10K+</div>
                <div className="text-sm text-neutral-600 font-medium">Research Papers</div>
              </div>
              <div className="text-center">
                <div className="text-3xl font-bold text-neutral-900">500+</div>
                <div className="text-sm text-neutral-600 font-medium">Journals</div>
              </div>
              <div className="text-center">
                <div className="text-3xl font-bold text-neutral-900">95%</div>
                <div className="text-sm text-neutral-600 font-medium">Accuracy</div>
              </div>
            </div>

            {/* Code Snippet */}
            <div className="w-full max-w-2xl">
              <div className="bg-surface border border-neutral-200 rounded-lg shadow-card overflow-hidden">
                <div className="flex items-center justify-between px-4 py-3 border-b border-neutral-100 bg-neutral-50">
                  <div className="flex items-center space-x-2">
                    <div className="w-3 h-3 bg-red-400 rounded-full"></div>
                    <div className="w-3 h-3 bg-yellow-400 rounded-full"></div>
                    <div className="w-3 h-3 bg-green-400 rounded-full"></div>
                  </div>
                  <button 
                    onClick={handleCopyCode}
                    className="flex items-center space-x-2 px-3 py-1 text-xs font-medium text-neutral-600 hover:text-neutral-800 hover:bg-neutral-100 rounded-md transition-colors"
                  >
                    {copied ? (
                      <>
                        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                        </svg>
                        <span>Copied!</span>
                      </>
                    ) : (
                      <>
                        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                        </svg>
                        <span>Copy</span>
                      </>
                    )}
                  </button>
                </div>
                <div className="p-4">
                  <pre 
                    ref={codeRef}
                    className={`font-mono text-sm text-neutral-800 leading-relaxed overflow-x-auto ${
                      codeExpanded ? '' : 'max-h-24 overflow-hidden'
                    }`}
                  >
                    <code>{codeSnippet}</code>
                  </pre>
                  {!codeExpanded && (
                    <button 
                      onClick={() => setCodeExpanded(true)}
                      className="mt-2 text-xs text-primary hover:text-primary-600 font-medium"
                    >
                      Show more...
                    </button>
                  )}
                </div>
              </div>
            </div>
          </div>

          {/* Right Visual */}
          <div className="hidden lg:block">
            <div className="relative">
              {/* Main visual container */}
              <div className="rounded-2xl bg-gradient-to-br from-neutral-50 to-neutral-100 p-8 shadow-pop">
                <div className="space-y-6">
                  {/* Mock dashboard header */}
                  <div className="flex items-center justify-between">
                    <div className="flex items-center space-x-3">
                      <div className="w-8 h-8 bg-primary rounded-lg flex items-center justify-center">
                        <svg className="w-5 h-5 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                        </svg>
                      </div>
                      <div>
                        <div className="text-sm font-semibold text-neutral-900">Research Dashboard</div>
                        <div className="text-xs text-neutral-600">AI-Powered Search</div>
                      </div>
                    </div>
                    <div className="w-3 h-3 bg-green-400 rounded-full"></div>
                  </div>

                  {/* Mock search results */}
                  <div className="space-y-4">
                    <div className="bg-white rounded-lg p-4 shadow-soft border border-neutral-200">
                      <div className="flex items-start space-x-3">
                        <div className="w-2 h-2 bg-primary rounded-full mt-2"></div>
                        <div className="flex-1">
                          <div className="text-sm font-medium text-neutral-900">Deep Learning Applications in Medical Imaging</div>
                          <div className="text-xs text-neutral-600 mt-1">Nature Medicine • 2024 • Q1 Journal</div>
                        </div>
                      </div>
                    </div>
                    
                    <div className="bg-white rounded-lg p-4 shadow-soft border border-neutral-200">
                      <div className="flex items-start space-x-3">
                        <div className="w-2 h-2 bg-green-400 rounded-full mt-2"></div>
                        <div className="flex-1">
                          <div className="text-sm font-medium text-neutral-900">Neural Networks for Drug Discovery</div>
                          <div className="text-xs text-neutral-600 mt-1">Science • 2024 • Q1 Journal</div>
                        </div>
                      </div>
                    </div>

                    <div className="bg-white rounded-lg p-4 shadow-soft border border-neutral-200">
                      <div className="flex items-start space-x-3">
                        <div className="w-2 h-2 bg-yellow-400 rounded-full mt-2"></div>
                        <div className="flex-1">
                          <div className="text-sm font-medium text-neutral-900">Machine Learning in Climate Research</div>
                          <div className="text-xs text-neutral-600 mt-1">Nature Climate Change • 2024 • Q1 Journal</div>
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* Mock metrics */}
                  <div className="grid grid-cols-2 gap-4 pt-4 border-t border-neutral-200">
                    <div className="text-center">
                      <div className="text-lg font-bold text-neutral-900">1,247</div>
                      <div className="text-xs text-neutral-600">Papers Found</div>
                    </div>
                    <div className="text-center">
                      <div className="text-lg font-bold text-primary">98.7%</div>
                      <div className="text-xs text-neutral-600">Relevance Score</div>
                    </div>
                  </div>
                </div>
              </div>

              {/* Floating elements */}
              <div className="absolute -top-4 -right-4 w-16 h-16 bg-primary/10 rounded-full flex items-center justify-center animate-float">
                <svg className="w-8 h-8 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
                </svg>
              </div>

              <div className="absolute -bottom-4 -left-4 w-12 h-12 bg-green-400/10 rounded-full flex items-center justify-center animate-float" style={{ animationDelay: '1s' }}>
                <svg className="w-6 h-6 text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}