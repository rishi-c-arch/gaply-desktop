import React from 'react';
import './QuartileAnalysis3D.css';

const buildConicGradient = (data: number[], colors: string[]) => {
  const total = data.reduce((sum, v) => sum + v, 0) || 1;
  let acc = 0;
  const stops = data.map((value, idx) => {
    const start = (acc / total) * 100;
    acc += value;
    const end = (acc / total) * 100;
    const color = colors[idx % colors.length];
    return `${color} ${start.toFixed(2)}% ${end.toFixed(2)}%`;
  });
  return `conic-gradient(${stops.join(', ')})`;
};

const QuartileAnalysis3D: React.FC = () => {
  // Premium grayscale palettes (Apple-style, neutral)
  const colorSchemes = {
    q1: ['#f5f5f5', '#e6e6e6', '#d6d6d6', '#c7c7c7'],
    q2: ['#ffffff', '#efefef', '#dedede', '#cdcdcd', '#bdbdbd'],
    q3: ['#ededed', '#dcdcdc', '#cbcbcb', '#bababa', '#a9a9a9'],
    q4: ['#f0f0f0', '#dfdfdf', '#cecece', '#bdbdbd']
  };

  const quartileData = {
    q1: [25, 25, 25, 25],
    q2: [20, 20, 20, 20, 20],
    q3: [20, 20, 20, 20, 20],
    q4: [25, 25, 25, 25]
  };

  return (
    <section className="qa-section" aria-label="Journal Quartile Analysis">
      <div className="qa-divider" />

      <header className="qa-header">
        <h2>Journal Quartile Analysis</h2>
        <div className="qa-header-accent" />
      </header>

      <div className="qa-grid">
        {[
          { quartile: 'q1', title: 'Q1', colors: colorSchemes.q1, data: quartileData.q1 },
          { quartile: 'q2', title: 'Q2', colors: colorSchemes.q2, data: quartileData.q2 },
          { quartile: 'q3', title: 'Q3', colors: colorSchemes.q3, data: quartileData.q3 },
          { quartile: 'q4', title: 'Q4', colors: colorSchemes.q4, data: quartileData.q4 }
        ].map((item) => (
          <button
            key={item.quartile}
            className="qa-card"
            onClick={() => window.open(`/reports/journal-${item.quartile}-analysis.html`, '_blank')}
            aria-label={`Open ${item.title} journal analysis`}
          >
            <div className="qa-card-glow" />
            <div className="qa-card-body">
              <div className="qa-card-title">{item.title}</div>
              <div
                className="qa-chart-wrap"
                style={{ '--qa-ring-gradient': buildConicGradient(item.data, item.colors) } as React.CSSProperties}
                aria-hidden="true"
              />
            </div>
            <div className="qa-card-border" />
          </button>
        ))}
      </div>
    </section>
  );
};

export default QuartileAnalysis3D;
