interface ReportData {
  status: string;
  report_id: string;
  journal_url?: string;
  manuscript_link?: string;
  chunks?: any[];
  publication_chance?: any;
  referee_review?: any;
  line_review?: string;
}

// Helper function to escape HTML
const escapeHtml = (text: string): string => {
  const map: { [key: string]: string } = {
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#039;'
  };
  return text.replace(/[&<>"']/g, (m) => map[m]);
};

export const generateHTMLReport = (reportData: ReportData, manuscriptTitle?: string): string => {
  const date = new Date().toLocaleDateString('en-US', { 
    year: 'numeric', 
    month: 'long', 
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit'
  });

  const getRefereeDecisionColor = (decision: string) => {
    switch (decision?.toLowerCase()) {
      case 'accept': return '#34c759';
      case 'minor_revision': return '#ff9500';
      case 'major_revision': return '#ff3b30';
      case 'reject': return '#ff3b30';
      default: return '#8e8e93';
    }
  };

  const getConfidenceColor = (confidence: number) => {
    if (confidence >= 0.8) return '#34c759';
    if (confidence >= 0.6) return '#ff9500';
    return '#ff3b30';
  };

  const formatSectionScore = (score: number) => {
    const color = score >= 80 ? '#34c759' : score >= 60 ? '#ff9500' : '#ff3b30';
    return `<span style="color: ${color}; font-weight: 600;">${score}/100</span>`;
  };

  const formatConfidence = (confidence: number) => {
    const percentage = Math.round(confidence * 100);
    const color = getConfidenceColor(confidence);
    return `<span style="color: ${color}; font-weight: 600;">${percentage}%</span>`;
  };

  // Generate meaningful section names from chunk text
  const generateSectionName = (chunk: any, index: number): string => {
    const chunkText = chunk.chunk_text || chunk.text || '';
    if (!chunkText) return `Section ${index + 1}`;
    
    // Extract first line
    const firstLine = chunkText.split('\n')[0].trim();
    if (firstLine.length > 0 && firstLine.length < 60) {
      // Check if it looks like a heading (all caps or capitalized first word)
      if (firstLine === firstLine.toUpperCase() || /^[A-Z][a-z]+/.test(firstLine)) {
        const trimmed = firstLine.length > 50 ? firstLine.slice(0, 47) + '...' : firstLine;
        // Clean up common section indicators
        return trimmed.replace(/^(Abstract|Introduction|Methods|Methodology|Results|Discussion|Conclusion|References|Appendix)[\s:]*/i, '');
      }
    }
    
    // Extract first meaningful sentence
    const sentences = chunkText.split(/[.!?]\s+/).filter((s: string) => s.trim().length > 20);
    if (sentences.length > 0) {
      const name = sentences[0].trim();
      return name.length > 50 ? name.slice(0, 47) + '...' : name;
    }
    
    // Fallback to section number
    return `Section ${index + 1}`;
  };

  const refereeData = reportData.referee_review || {};
  const publicationData = reportData.publication_chance || {};
  const acceptanceProb = refereeData.acceptance_probability || 0;
  const decision = refereeData.decision || 'unknown';

  const html = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Document Analysis Report - ${reportData.report_id}</title>
  <style>
    * {
      margin: 0;
      padding: 0;
      box-sizing: border-box;
    }
    
    body {
      font-family: -apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif;
      background: linear-gradient(135deg, #1a1a1f 0%, #0f0f14 100%);
      color: #f5f5f7;
      line-height: 1.6;
      padding: 40px 20px;
    }
    
    .container {
      max-width: 1200px;
      margin: 0 auto;
      background: rgba(255, 255, 255, 0.05);
      border-radius: 24px;
      padding: 48px;
      box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
      border: 1px solid rgba(255, 255, 255, 0.1);
    }
    
    .header {
      text-align: center;
      margin-bottom: 48px;
      padding-bottom: 32px;
      border-bottom: 2px solid rgba(255, 255, 255, 0.1);
    }
    
    .header h1 {
      font-size: 2.5rem;
      font-weight: 700;
      margin-bottom: 12px;
      background: linear-gradient(135deg, #ffffff 0%, #a0a0a5 100%);
      -webkit-background-clip: text;
      -webkit-text-fill-color: transparent;
      background-clip: text;
    }
    
    .header .subtitle {
      font-size: 1.1rem;
      color: rgba(255, 255, 255, 0.7);
      margin-bottom: 8px;
    }
    
    .header .meta {
      font-size: 0.9rem;
      color: rgba(255, 255, 255, 0.5);
    }
    
    .section {
      margin-bottom: 48px;
    }
    
    .section-title {
      font-size: 1.75rem;
      font-weight: 600;
      margin-bottom: 24px;
      color: #ffffff;
      padding-bottom: 12px;
      border-bottom: 2px solid rgba(255, 255, 255, 0.15);
    }
    
    .summary-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
      gap: 20px;
      margin-bottom: 32px;
    }
    
    .summary-card {
      background: rgba(255, 255, 255, 0.08);
      border: 1px solid rgba(255, 255, 255, 0.1);
      border-radius: 16px;
      padding: 24px;
      transition: transform 0.2s, box-shadow 0.2s;
    }
    
    .summary-card:hover {
      transform: translateY(-2px);
      box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
    }
    
    .summary-card-title {
      font-size: 0.85rem;
      text-transform: uppercase;
      letter-spacing: 0.5px;
      color: rgba(255, 255, 255, 0.6);
      margin-bottom: 12px;
    }
    
    .summary-card-value {
      font-size: 2rem;
      font-weight: 700;
      color: #ffffff;
      margin-bottom: 8px;
    }
    
    .summary-card-desc {
      font-size: 0.9rem;
      color: rgba(255, 255, 255, 0.7);
      line-height: 1.5;
    }
    
    .decision-badge {
      display: inline-block;
      padding: 8px 16px;
      border-radius: 20px;
      font-size: 0.9rem;
      font-weight: 600;
      text-transform: uppercase;
      letter-spacing: 0.5px;
      background: ${getRefereeDecisionColor(decision)}22;
      color: ${getRefereeDecisionColor(decision)};
      border: 1px solid ${getRefereeDecisionColor(decision)}44;
    }
    
    .score-bar {
      background: rgba(255, 255, 255, 0.1);
      border-radius: 8px;
      height: 24px;
      margin: 8px 0;
      overflow: hidden;
      position: relative;
    }
    
    .score-fill {
      height: 100%;
      border-radius: 8px;
      transition: width 0.3s ease;
      background: linear-gradient(90deg, #34c759 0%, #30d158 100%);
    }
    
    .score-fill.warning {
      background: linear-gradient(90deg, #ff9500 0%, #ffaa00 100%);
    }
    
    .score-fill.danger {
      background: linear-gradient(90deg, #ff3b30 0%, #ff6b6b 100%);
    }
    
    .score-label {
      position: absolute;
      top: 50%;
      left: 12px;
      transform: translateY(-50%);
      font-size: 0.85rem;
      font-weight: 600;
      color: #ffffff;
    }
    
    .issues-list {
      list-style: none;
      padding: 0;
    }
    
    .issue-item {
      background: rgba(255, 255, 255, 0.05);
      border-left: 4px solid;
      border-radius: 8px;
      padding: 16px 20px;
      margin-bottom: 16px;
      transition: background 0.2s;
    }
    
    .issue-item:hover {
      background: rgba(255, 255, 255, 0.08);
    }
    
    .issue-item.major {
      border-left-color: #ff3b30;
    }
    
    .issue-item.minor {
      border-left-color: #ff9500;
    }
    
    .issue-category {
      font-size: 0.85rem;
      text-transform: uppercase;
      letter-spacing: 0.5px;
      color: rgba(255, 255, 255, 0.6);
      margin-bottom: 8px;
    }
    
    .issue-evidence {
      color: rgba(255, 255, 255, 0.8);
      margin: 8px 0;
      font-style: italic;
    }
    
    .issue-recommendation {
      color: rgba(255, 255, 255, 0.9);
      margin-top: 8px;
      font-weight: 500;
    }
    
    .chunk-card {
      background: rgba(255, 255, 255, 0.05);
      border: 1px solid rgba(255, 255, 255, 0.1);
      border-radius: 12px;
      padding: 20px;
      margin-bottom: 20px;
    }
    
    .chunk-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 16px;
      padding-bottom: 12px;
      border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    }
    
    .chunk-id {
      font-size: 0.9rem;
      color: rgba(255, 255, 255, 0.7);
      font-weight: 500;
    }
    
    .task-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
      gap: 16px;
      margin-top: 16px;
    }
    
    .task-card {
      background: rgba(0, 0, 0, 0.2);
      border-radius: 8px;
      padding: 16px;
      border: 1px solid rgba(255, 255, 255, 0.1);
    }
    
    .task-title {
      font-size: 0.85rem;
      text-transform: uppercase;
      letter-spacing: 0.5px;
      color: rgba(255, 255, 255, 0.6);
      margin-bottom: 8px;
    }
    
    .task-status {
      display: inline-block;
      padding: 4px 10px;
      border-radius: 12px;
      font-size: 0.75rem;
      font-weight: 600;
      text-transform: uppercase;
      margin-bottom: 8px;
    }
    
    .task-status.ok {
      background: #34c75922;
      color: #34c759;
    }
    
    .task-status.flagged {
      background: #ff950022;
      color: #ff9500;
    }
    
    .task-status.passed {
      background: #34c75922;
      color: #34c759;
    }
    
    .task-summary {
      font-size: 0.9rem;
      color: rgba(255, 255, 255, 0.8);
      margin-top: 8px;
      line-height: 1.5;
    }
    
    .methodology-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
      gap: 16px;
      margin: 24px 0;
    }
    
    .methodology-item {
      text-align: center;
      padding: 20px;
      background: rgba(255, 255, 255, 0.05);
      border-radius: 12px;
    }
    
    .methodology-label {
      font-size: 0.85rem;
      color: rgba(255, 255, 255, 0.6);
      margin-bottom: 8px;
    }
    
    .methodology-score {
      font-size: 2rem;
      font-weight: 700;
      color: #34c759;
    }
    
    table {
      width: 100%;
      border-collapse: collapse;
      margin: 20px 0;
      background: rgba(0, 0, 0, 0.2);
      border-radius: 8px;
      overflow: hidden;
    }
    
    th, td {
      padding: 12px 16px;
      text-align: left;
      border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    }
    
    th {
      background: rgba(255, 255, 255, 0.1);
      font-weight: 600;
      color: #ffffff;
      text-transform: uppercase;
      font-size: 0.85rem;
      letter-spacing: 0.5px;
    }
    
    tr:hover {
      background: rgba(255, 255, 255, 0.05);
    }
    
    .footer {
      text-align: center;
      margin-top: 48px;
      padding-top: 32px;
      border-top: 2px solid rgba(255, 255, 255, 0.1);
      color: rgba(255, 255, 255, 0.5);
      font-size: 0.85rem;
    }
    
    @media print {
      body {
        background: white;
        color: #000;
      }
      
      .container {
        background: white;
        box-shadow: none;
        border: 1px solid #ddd;
      }
    }
  </style>
</head>
<body>
  <div class="container">
    <div class="header">
      <h1>Document Analysis Report</h1>
      <p class="subtitle">${manuscriptTitle || 'Manuscript Evaluation'}</p>
      <p class="meta">Report ID: ${reportData.report_id} | Generated: ${date}</p>
      ${reportData.journal_url ? `<p class="meta">Journal: <a href="${reportData.journal_url}" style="color: #007AFF;">${reportData.journal_url}</a></p>` : ''}
    </div>

    <!-- Executive Summary -->
    <div class="section">
      <h2 class="section-title">Executive Summary</h2>
      <div class="summary-grid">
        <div class="summary-card">
          <div class="summary-card-title">Referee Decision</div>
          <div class="summary-card-value">
            <span class="decision-badge">${decision.replace(/_/g, ' ')}</span>
          </div>
          <div class="summary-card-desc">Acceptance Probability: ${acceptanceProb}%</div>
        </div>
        
        <div class="summary-card">
          <div class="summary-card-title">Publication Chance</div>
          <div class="summary-card-value">${publicationData.estimated_publication_chance || 'N/A'}</div>
          <div class="summary-card-desc">${publicationData.reasoning ? publicationData.reasoning.slice(0, 100) + '...' : 'Analysis complete'}</div>
        </div>
        
        <div class="summary-card">
          <div class="summary-card-title">Total Chunks Analyzed</div>
          <div class="summary-card-value">${reportData.chunks?.length || 0}</div>
          <div class="summary-card-desc">Deep evaluation completed</div>
        </div>
      </div>
    </div>

    ${refereeData.section_scores ? `
    <!-- Section Scores -->
    <div class="section">
      <h2 class="section-title">Section Scores</h2>
      <div class="methodology-grid">
        ${Object.entries(refereeData.section_scores).map(([section, score]: [string, any]) => `
          <div class="methodology-item">
            <div class="methodology-label">${section.replace(/_/g, ' ')}</div>
            <div class="methodology-score" style="color: ${score >= 80 ? '#34c759' : score >= 60 ? '#ff9500' : '#ff3b30'}">${score}/100</div>
          </div>
        `).join('')}
      </div>
    </div>
    ` : ''}

    ${refereeData.methodology_rubric ? `
    <!-- Methodology Rubric -->
    <div class="section">
      <h2 class="section-title">Methodology Rubric</h2>
      <div class="methodology-grid">
        ${Object.entries(refereeData.methodology_rubric).filter(([key]) => key !== 'notes').map(([key, score]: [string, any]) => `
          <div class="methodology-item">
            <div class="methodology-label">${key.replace(/_/g, ' ')}</div>
            <div class="methodology-score" style="color: #34c759">${score}/5</div>
          </div>
        `).join('')}
      </div>
      ${refereeData.methodology_rubric.notes ? `<p style="margin-top: 20px; color: rgba(255, 255, 255, 0.7); line-height: 1.6;">${refereeData.methodology_rubric.notes}</p>` : ''}
    </div>
    ` : ''}

    ${refereeData.issues && refereeData.issues.length > 0 ? `
    <!-- Key Issues -->
    <div class="section">
      <h2 class="section-title">Key Issues & Recommendations</h2>
      <ul class="issues-list">
        ${refereeData.issues.map((issue: any) => `
          <li class="issue-item ${issue.severity || 'minor'}">
            <div class="issue-category">${issue.category || 'General'}</div>
            <div class="issue-evidence">${issue.evidence || ''}</div>
            <div class="issue-recommendation"><strong>Recommendation:</strong> ${issue.recommendation || ''}</div>
          </li>
        `).join('')}
      </ul>
    </div>
    ` : ''}

    ${refereeData.what_to_fix && refereeData.what_to_fix.length > 0 ? `
    <!-- What to Fix -->
    <div class="section">
      <h2 class="section-title">What to Fix</h2>
      <ul style="list-style: none; padding: 0;">
        ${refereeData.what_to_fix.map((fix: string) => `
          <li style="background: rgba(255, 255, 255, 0.05); padding: 16px; margin-bottom: 12px; border-radius: 8px; border-left: 4px solid #ff9500;">
            ${fix}
          </li>
        `).join('')}
      </ul>
    </div>
    ` : ''}

    ${refereeData.how_to_fix && refereeData.how_to_fix.length > 0 ? `
    <!-- How to Fix -->
    <div class="section">
      <h2 class="section-title">How to Fix</h2>
      <ul style="list-style: none; padding: 0;">
        ${refereeData.how_to_fix.map((fix: string) => `
          <li style="background: rgba(255, 255, 255, 0.05); padding: 16px; margin-bottom: 12px; border-radius: 8px; border-left: 4px solid #34c759;">
            ${fix}
          </li>
        `).join('')}
      </ul>
    </div>
    ` : ''}

    ${refereeData.evidence_table && refereeData.evidence_table.length > 0 ? `
    <!-- Evidence Table -->
    <div class="section">
      <h2 class="section-title">Evidence Table</h2>
      <table>
        <thead>
          <tr>
            <th>Claim</th>
            <th>Evidence Type</th>
            <th>Strength</th>
            <th>Recommendation</th>
          </tr>
        </thead>
        <tbody>
          ${refereeData.evidence_table.map((item: any) => `
            <tr>
              <td>${item.claim || ''}</td>
              <td><span style="text-transform: uppercase; font-size: 0.85rem;">${item.evidence_type || ''}</span></td>
              <td><span style="padding: 4px 10px; border-radius: 12px; background: ${item.strength === 'strong' ? '#34c75922' : '#ff950022'}; color: ${item.strength === 'strong' ? '#34c759' : '#ff9500'}; font-size: 0.75rem; font-weight: 600;">${item.strength || ''}</span></td>
              <td>${item.recommendation || ''}</td>
            </tr>
          `).join('')}
        </tbody>
      </table>
    </div>
    ` : ''}

    ${reportData.chunks && reportData.chunks.length > 0 ? `
    <!-- Chunk Analysis -->
    <div class="section">
      <h2 class="section-title">Detailed Chunk Analysis</h2>
      ${reportData.chunks.slice(0, 10).map((chunk: any, idx: number) => {
        const sectionName = generateSectionName(chunk, idx);
        return `
        <div class="chunk-card">
          <div class="chunk-header">
            <span class="chunk-id">${escapeHtml(sectionName)}</span>
            <span style="font-size: 0.85rem; color: rgba(255, 255, 255, 0.6);">${chunk.status || 'ok'}</span>
          </div>
          ${chunk.task_results ? `
            <div class="task-grid">
              ${Object.entries(chunk.task_results).map(([taskName, taskData]: [string, any]) => `
                <div class="task-card">
                  <div class="task-title">${taskName.replace(/_/g, ' ')}</div>
                  <div class="task-status ${taskData.task_status || 'ok'}">${taskData.task_status || 'ok'}</div>
                  <div style="margin: 8px 0; color: rgba(255, 255, 255, 0.7); font-size: 0.85rem;">Confidence: ${formatConfidence(taskData.confidence || 0)}</div>
                  <div class="task-summary">${taskData.summary || ''}</div>
                </div>
              `).join('')}
            </div>
          ` : ''}
        </div>
      `;
      }).join('')}
      ${reportData.chunks.length > 10 ? `<p style="text-align: center; color: rgba(255, 255, 255, 0.6); margin-top: 20px;">... and ${reportData.chunks.length - 10} more chunks</p>` : ''}
    </div>
    ` : ''}

    <div class="footer">
      <p>Generated by Gaply Document Analysis Orchestrator</p>
      <p>Powered by OpenAI GPT-4.1 | ${date}</p>
    </div>
  </div>
</body>
</html>`;

  return html;
};

export const downloadHTMLReport = (reportData: ReportData, filename?: string) => {
  const html = generateHTMLReport(reportData);
  const blob = new Blob([html], { type: 'text/html' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename || `gaply-report-${reportData.report_id}.html`;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);
};
