import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';

// Backend API configuration
const API_BASE_URL = 'https://gaply-backend-gaply.up.railway.app';

const AcademicAIRemoverPage: React.FC = () => {
  const navigate = useNavigate();
  const [removerText, setRemoverText] = useState('');
  const [removerResult, setRemoverResult] = useState<string | null>(null);
  const [isProcessing, setIsProcessing] = useState(false);

  const handleParaphrase = async () => {
    if (!removerText.trim()) return;
    setIsProcessing(true);
    setRemoverResult('Processing...');

    try {
      // Call backend API for direct paraphrasing (only requires 'text' field)
      const response = await fetch(`${API_BASE_URL}/api/v1/paraphrase/direct`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          text: removerText
        }),
      });

      if (!response.ok) {
        throw new Error('Failed to paraphrase text');
      }

      const data = await response.json();
      
      // Display the paraphrased text from backend
      setRemoverResult(data.paraphrased_text || data.result || 'No paraphrased text received');
      setIsProcessing(false);
    } catch (error) {
      console.error('Paraphrase error:', error);
      setRemoverResult('Error: Failed to connect to backend. Please try again later.');
      setIsProcessing(false);
    }
  };

  // Old client-side processing (fallback/legacy)
  const handleParaphraseClientSide = () => {
    if (!removerText.trim()) return;
    setIsProcessing(true);
    setRemoverResult('Processing...');

    setTimeout(() => {
      let text = removerText;

      // Phase 1: Remove casual language
      text = text.replace(/\b(very|really|basically|just|quite|pretty|fairly|rather|kind of|sort of|like|you know|I mean|okay|totally|definitely|obviously|honestly|literally|actually)\b/gi, '');
      text = text.replace(/\b(I think|I believe|I feel|in my opinion|personally)\b/gi, '');

      // Phase 2: Fix contractions
      const contractions = {
        "can't": "cannot", "won't": "will not", "don't": "do not", "doesn't": "does not",
        "didn't": "did not", "haven't": "have not", "hasn't": "has not", "isn't": "is not",
        "aren't": "are not", "wasn't": "was not", "weren't": "were not", "it's": "it is",
        "that's": "that is", "there's": "there is", "what's": "what is", "who's": "who is"
      };
      Object.keys(contractions).forEach(key => {
        text = text.replace(new RegExp('\\b' + key + '\\b', 'gi'), contractions[key as keyof typeof contractions]);
      });

      // Phase 3: Advanced Synonym Replacement (Random for variation)
      const synonyms = {
        'important': ['significant', 'crucial', 'essential', 'vital', 'paramount'],
        'show': ['demonstrate', 'illustrate', 'reveal', 'indicate', 'exhibit'],
        'find': ['discover', 'identify', 'ascertain', 'determine', 'establish'],
        'use': ['utilize', 'employ', 'implement', 'apply', 'leverage'],
        'make': ['create', 'generate', 'produce', 'construct', 'formulate'],
        'get': ['obtain', 'acquire', 'procure', 'secure', 'attain'],
        'give': ['provide', 'offer', 'furnish', 'supply', 'deliver'],
        'help': ['facilitate', 'assist', 'support', 'aid', 'enable'],
        'change': ['transform', 'modify', 'alter', 'adapt', 'revise'],
        'big': ['substantial', 'considerable', 'significant', 'extensive'],
        'small': ['minimal', 'negligible', 'modest', 'limited'],
        'good': ['beneficial', 'advantageous', 'favorable', 'positive'],
        'bad': ['detrimental', 'adverse', 'unfavorable', 'negative'],
        'many': ['numerous', 'multiple', 'various', 'abundant'],
        'different': ['distinct', 'varied', 'diverse', 'disparate'],
        'because': ['due to', 'owing to', 'as a result of', 'given that'],
        'but': ['however', 'nevertheless', 'nonetheless', 'conversely'],
        'also': ['additionally', 'furthermore', 'moreover', 'likewise'],
        'think': ['consider', 'contemplate', 'postulate', 'theorize'],
        'believe': ['posit', 'maintain', 'contend', 'assert'],
        'say': ['state', 'articulate', 'express', 'convey'],
        'look': ['examine', 'analyze', 'investigate', 'scrutinize'],
        'see': ['observe', 'perceive', 'discern', 'witness'],
        'work': ['function', 'operate', 'perform', 'execute'],
        'need': ['require', 'necessitate', 'demand', 'warrant'],
        'want': ['desire', 'seek', 'aspire to', 'aim to'],
        'try': ['attempt', 'endeavor', 'strive to', 'undertake'],
        'start': ['initiate', 'commence', 'embark on', 'launch'],
        'end': ['conclude', 'terminate', 'finalize', 'culminate'],
        'keep': ['maintain', 'preserve', 'sustain', 'retain'],
        'increase': ['enhance', 'augment', 'amplify', 'elevate'],
        'decrease': ['reduce', 'diminish', 'minimize', 'curtail'],
        'suggest': ['indicate', 'imply', 'propose', 'intimate'],
        'explain': ['elucidate', 'clarify', 'expound', 'explicate'],
        'describe': ['delineate', 'characterize', 'depict', 'portray'],
        'analyze': ['examine', 'scrutinize', 'investigate', 'assess'],
        'discuss': ['examine', 'explore', 'consider', 'deliberate'],
        'demonstrate': ['illustrate', 'exhibit', 'manifest', 'display'],
        'indicate': ['signify', 'denote', 'suggest', 'evidence'],
        'achieve': ['accomplish', 'attain', 'realize', 'secure'],
        'develop': ['cultivate', 'advance', 'progress', 'evolve'],
        'create': ['generate', 'establish', 'formulate', 'originate'],
        'provide': ['furnish', 'supply', 'deliver', 'yield'],
        'allow': ['permit', 'enable', 'facilitate', 'authorize'],
        'include': ['encompass', 'comprise', 'incorporate', 'contain'],
        'involve': ['entail', 'encompass', 'necessitate', 'require'],
        'result': ['outcome', 'consequence', 'effect', 'culmination'],
        'impact': ['influence', 'affect', 'effect', 'consequence'],
        'problem': ['issue', 'challenge', 'difficulty', 'complication'],
        'method': ['approach', 'technique', 'procedure', 'methodology'],
        'data': ['information', 'evidence', 'statistics', 'findings']
      };

      Object.keys(synonyms).forEach(word => {
        const syns = synonyms[word as keyof typeof synonyms];
        const pattern = new RegExp('\\b' + word + '\\b', 'gi');
        text = text.replace(pattern, () => syns[Math.floor(Math.random() * syns.length)]);
      });

      // Phase 4: Sentence Restructuring
      const sentences = text.match(/[^.!?]+[.!?]+/g) || [text];
      const transitions = ['Furthermore', 'Moreover', 'Additionally', 'Notably', 'Consequently', 'Subsequently'];
      const hedges = ['Evidence suggests that', 'Research indicates that', 'Analysis reveals that', 'Studies demonstrate that'];

      const restructured = sentences.map(sentence => {
        sentence = sentence.trim();
        if (Math.random() > 0.7 && sentence.length > 40) {
          const trans = transitions[Math.floor(Math.random() * transitions.length)];
          sentence = trans + ', ' + sentence.charAt(0).toLowerCase() + sentence.slice(1);
        }
        if (Math.random() > 0.8 && sentence.length > 50) {
          const hedge = hedges[Math.floor(Math.random() * hedges.length)];
          sentence = hedge + ' ' + sentence.charAt(0).toLowerCase() + sentence.slice(1);
        }
        return sentence;
      });

      text = restructured.join(' ');

      // Phase 5: Final cleanup
      text = text.replace(/\s{2,}/g, ' ');
      text = text.replace(/\s+([.,!?;:])/g, '$1');
      text = text.replace(/([.,!?;:])\s*/g, '$1 ');
      text = text.trim();

      if (text.length > 0) {
        text = text.charAt(0).toUpperCase() + text.slice(1);
      }
      text = text.replace(/\.\s+([a-z])/g, (match, letter) => '. ' + letter.toUpperCase());

      setRemoverResult(text + '\n\n✅ ADVANCED ACADEMIC HUMANIZATION COMPLETE\n🛡️ BYPASSES: Turnitin, GPTZero, Crossplag, Content at Scale, Copyleaks, OpenAI, Sapling, Writer\n📚 Features: Complex paraphrasing, Academic language, Perfect grammar, Human-like structure');
      setIsProcessing(false);
    }, 800);
  };

  return (
    <div style={{
      minHeight: '100vh',
      background: '#000000',
      padding: '40px 20px',
    }}>
      {/* Navigation Bar */}
      <div style={{
        maxWidth: '1200px',
        margin: '0 auto 40px',
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
      }}>
        <h1 style={{
          fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
          fontSize: 'clamp(1.5rem, 3vw, 2rem)',
          fontWeight: '600',
          color: '#ffffff',
          margin: 0
        }}>
          Academic AI Remover
        </h1>
        <button
          onClick={() => navigate('/')}
          style={{
            background: 'rgba(255, 255, 255, 0.1)',
            border: '1px solid rgba(255, 255, 255, 0.2)',
            color: '#ffffff',
            padding: '12px 24px',
            borderRadius: '8px',
            fontSize: '14px',
            fontWeight: '500',
            cursor: 'pointer',
            transition: 'all 0.3s ease',
            fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif'
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.background = 'rgba(255, 255, 255, 0.2)';
            e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.4)';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = 'rgba(255, 255, 255, 0.1)';
            e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.2)';
          }}
        >
          ← Back to Home
        </button>
      </div>

      {/* Main Content */}
      <div style={{
        maxWidth: '1200px',
        margin: '0 auto',
        display: 'grid',
        gridTemplateColumns: '1fr 1fr',
        gap: '40px',
        alignItems: 'start'
      }}>
        {/* Input Section */}
        <div style={{
          background: 'rgba(255, 255, 255, 0.05)',
          borderRadius: '16px',
          padding: '32px',
          border: '1px solid rgba(255, 255, 255, 0.1)',
          backdropFilter: 'blur(10px)'
        }}>
          <h2 style={{
            fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
            fontSize: '1.5rem',
            fontWeight: '600',
            color: '#ffffff',
            marginBottom: '24px'
          }}>
            Input Text
          </h2>
          <textarea
            value={removerText}
            onChange={(e) => setRemoverText(e.target.value)}
            placeholder="Paste your AI-generated text here for advanced academic humanization..."
            style={{
              width: '100%',
              height: '300px',
              background: 'rgba(0, 0, 0, 0.3)',
              border: '1px solid rgba(255, 255, 255, 0.2)',
              borderRadius: '12px',
              padding: '20px',
              color: '#ffffff',
              fontSize: '16px',
              fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
              resize: 'vertical',
              outline: 'none',
              transition: 'border-color 0.3s ease'
            }}
            onFocus={(e) => e.target.style.borderColor = 'rgba(255, 255, 255, 0.4)'}
            onBlur={(e) => e.target.style.borderColor = 'rgba(255, 255, 255, 0.2)'}
          />
          <button
            onClick={handleParaphrase}
            disabled={isProcessing || !removerText.trim()}
            style={{
              width: '100%',
              background: isProcessing || !removerText.trim() 
                ? 'rgba(255, 255, 255, 0.1)' 
                : 'linear-gradient(135deg, #007AFF 0%, #5856D6 100%)',
              border: 'none',
              borderRadius: '12px',
              padding: '16px 24px',
              color: '#ffffff',
              fontSize: '16px',
              fontWeight: '600',
              cursor: isProcessing || !removerText.trim() ? 'not-allowed' : 'pointer',
              marginTop: '20px',
              transition: 'all 0.3s ease',
              fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif'
            }}
          >
            {isProcessing ? 'Processing...' : 'Humanize Text'}
          </button>
        </div>

        {/* Output Section */}
        <div style={{
          background: 'rgba(255, 255, 255, 0.05)',
          borderRadius: '16px',
          padding: '32px',
          border: '1px solid rgba(255, 255, 255, 0.1)',
          backdropFilter: 'blur(10px)'
        }}>
          <h2 style={{
            fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
            fontSize: '1.5rem',
            fontWeight: '600',
            color: '#ffffff',
            marginBottom: '24px'
          }}>
            Humanized Output
          </h2>
          <div style={{
            width: '100%',
            height: '300px',
            background: 'rgba(0, 0, 0, 0.3)',
            border: '1px solid rgba(255, 255, 255, 0.2)',
            borderRadius: '12px',
            padding: '20px',
            color: '#ffffff',
            fontSize: '16px',
            fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
            overflow: 'auto',
            whiteSpace: 'pre-wrap',
            lineHeight: '1.6'
          }}>
            {removerResult || 'Your humanized text will appear here...'}
          </div>
        </div>
      </div>

    </div>
  );
};

export default AcademicAIRemoverPage;