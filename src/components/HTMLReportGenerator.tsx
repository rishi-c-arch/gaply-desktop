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

  // Use EXACT section name from manuscript (backend or extracted from chunk text)
  const generateSectionName = (chunk: any, index: number): string => {
    // 1. Prefer section_name from backend (AI-identified from manuscript)
    const backendSection = chunk.section_name?.trim();
    if (backendSection) return backendSection;

    const chunkText = chunk.chunk_text || chunk.text || '';
    if (!chunkText) return `Section ${index + 1}`;

    const lines = chunkText.split('\n').map((l: string) => l.trim()).filter(Boolean);
    if (lines.length === 0) return `Section ${index + 1}`;

    const firstLine = lines[0];

    // 2. Known section headings (use EXACT text from manuscript)
    const sectionPattern = /^(ABSTRACT|INTRODUCTION|METHODS|METHODOLOGY|RESULTS|DISCUSSION|CONCLUSION|REFERENCES|APPENDIX|LITERATURE\s+REVIEW|KEYWORDS|ACKNOWLEDGMENTS|DATA\s+AVAILABILITY|AUTHOR\s+CONTRIBUTIONS|FUNDING|CONFLICT\s+OF\s+INTEREST|ETHICS|SUPPLEMENTARY|FIGURE\s+LEGENDS?|TABLES?|NOTES?)([\s:].*)?$/i;
    const sectionMatch = firstLine.match(sectionPattern);
    if (sectionMatch) {
      const name = sectionMatch[1].trim();
      return name.charAt(0).toUpperCase() + name.slice(1).toLowerCase().replace(/\s+/g, ' ');
    }

    // 3. Numbered headings: "1. Introduction", "2.1 Methods", "I. Background"
    const numberedMatch = firstLine.match(/^(\d+(\.\d+)*\.?\s*|[IVX]+\.\s*|[A-Z]\.\s*)(.+)$/);
    if (numberedMatch && numberedMatch[3].length < 80) {
      return (numberedMatch[1] + numberedMatch[3]).trim();
    }

    // 4. All-caps short line (likely heading)
    if (firstLine === firstLine.toUpperCase() && firstLine.length > 4 && firstLine.length < 100) {
      return firstLine.length > 60 ? firstLine.slice(0, 57) + '...' : firstLine;
    }

    // 5. Title case short line (likely heading)
    if (firstLine.length < 80 && /^[A-Z]/.test(firstLine)) {
      const words = firstLine.split(/\s+/);
      const capsCount = words.filter((w: string) => /^[A-Z]/.test(w)).length;
      if (capsCount >= words.length * 0.5) {
        return firstLine.length > 60 ? firstLine.slice(0, 57) + '...' : firstLine;
      }
    }

    // 6. First meaningful sentence
    const sentences = chunkText.split(/[.!?]\s+/).filter((s: string) => s.trim().length > 15);
    if (sentences.length > 0) {
      const name = sentences[0].trim();
      return name.length > 55 ? name.slice(0, 52) + '...' : name;
    }

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
    * { margin: 0; padding: 0; box-sizing: border-box; }
    
    body {
      font-family: "Times New Roman", Times, Georgia, serif;
      font-size: 9pt;
      line-height: 1.45;
      color: #1a1a1a;
      background: #fafaf8;
      padding: 2cm 1.75cm;
    }
    
    .container {
      max-width: 21cm;
      margin: 0 auto;
      background: #ffffff;
      padding: 2cm;
      border: 1px solid #d4d4d4;
      box-shadow: 0 1px 3px rgba(0,0,0,0.06);
    }
    
    .header {
      text-align: center;
      margin-bottom: 2em;
      padding-bottom: 1.5em;
      border-bottom: 1pt solid #1a1a1a;
    }
    
    .header h1 {
      font-size: 11pt;
      font-weight: 700;
      margin-bottom: 0.4em;
      color: #1a1a1a;
      letter-spacing: 0.03em;
    }
    
    .header .subtitle {
      font-size: 8.5pt;
      color: #444;
      margin-bottom: 0.2em;
    }
    
    .header .meta {
      font-size: 8pt;
      color: #666;
    }
    
    .section {
      margin-bottom: 2em;
    }
    
    .section-title {
      font-size: 9.5pt;
      font-weight: 700;
      margin-bottom: 0.75em;
      color: #1a1a1a;
      padding-bottom: 0.3em;
      border-bottom: 1pt solid #1a1a1a;
      text-transform: uppercase;
      letter-spacing: 0.06em;
    }
    
    .summary-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
      gap: 1em;
      margin-bottom: 1.5em;
    }
    
    .summary-card {
      background: #f8f8f6;
      border: 1px solid #e0e0dc;
      padding: 1em 1.25em;
    }
    
    .summary-card-title {
      font-size: 7pt;
      text-transform: uppercase;
      letter-spacing: 0.1em;
      color: #555;
      margin-bottom: 0.4em;
    }
    
    .summary-card-value {
      font-size: 12pt;
      font-weight: 700;
      color: #1a1a1a;
      margin-bottom: 0.25em;
    }
    
    .summary-card-desc {
      font-size: 9pt;
      color: #555;
      line-height: 1.4;
    }
    
    .decision-badge {
      display: inline-block;
      padding: 0.25em 0.6em;
      font-size: 8pt;
      font-weight: 600;
      text-transform: uppercase;
      letter-spacing: 0.08em;
      background: ${getRefereeDecisionColor(decision)}22;
      color: ${getRefereeDecisionColor(decision)};
      border: 1px solid ${getRefereeDecisionColor(decision)};
    }
    
    .score-bar {
      background: #e8e8e6;
      height: 18px;
      margin: 0.5em 0;
      overflow: hidden;
      position: relative;
    }
    
    .score-fill {
      height: 100%;
      background: #2d5a27;
    }
    
    .score-fill.warning { background: #8a6d00; }
    .score-fill.danger { background: #8b2e2e; }
    
    .score-label {
      position: absolute;
      top: 50%;
      left: 8px;
      transform: translateY(-50%);
      font-size: 8pt;
      font-weight: 600;
      color: #fff;
    }
    
    .issues-list { list-style: none; padding: 0; }
    
    .issue-item {
      background: #fafaf8;
      border-left: 3px solid #555;
      padding: 0.75em 1em;
      margin-bottom: 0.75em;
    }
    
    .issue-item.major { border-left-color: #8b2e2e; }
    .issue-item.minor { border-left-color: #8a6d00; }
    
    .issue-category {
      font-size: 8pt;
      text-transform: uppercase;
      letter-spacing: 0.08em;
      color: #555;
      margin-bottom: 0.35em;
    }
    
    .issue-evidence {
      color: #333;
      margin: 0.35em 0;
      font-style: italic;
      font-size: 9.5pt;
    }
    
    .issue-recommendation {
      color: #1a1a1a;
      margin-top: 0.35em;
      font-weight: 500;
      font-size: 9.5pt;
    }
    
    .chunk-card {
      background: #fafaf8;
      border: 1px solid #e0e0dc;
      padding: 1.25em 1.5em;
      margin-bottom: 1.5em;
    }
    
    .chunk-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 1em;
      padding-bottom: 0.5em;
      border-bottom: 1px solid #d4d4d4;
    }
    
    .chunk-id {
      font-size: 10pt;
      color: #1a1a1a;
      font-weight: 600;
    }
    
    .task-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
      gap: 1em;
      margin-top: 1em;
    }
    
    .task-card {
      background: #fff;
      border: 1px solid #e0e0dc;
      padding: 1em 1.25em;
    }
    
    .task-title {
      font-size: 8pt;
      text-transform: uppercase;
      letter-spacing: 0.08em;
      color: #555;
      margin-bottom: 8px;
    }
    
    .task-status {
      display: inline-block;
      padding: 0.2em 0.5em;
      font-size: 7pt;
      font-weight: 600;
      text-transform: uppercase;
      letter-spacing: 0.05em;
      margin-bottom: 0.5em;
    }
    
    .task-status.ok, .task-status.passed {
      background: #e8f0e8;
      color: #2d5a27;
    }
    
    .task-status.flagged {
      background: #f5f0e0;
      color: #8a6d00;
    }
    
    .task-summary {
      font-size: 9.5pt;
      color: #333;
      margin-top: 0.5em;
      line-height: 1.5;
    }
    
    .task-confidence {
      margin: 0.5em 0;
      color: #555;
      font-size: 8pt;
    }
    
    .task-details, .task-details-list {
      margin-top: 0.75em;
      padding-top: 0.75em;
      border-top: 1px solid #e0e0dc;
      font-size: 9pt;
      color: #444;
      line-height: 1.5;
    }
    
    .task-details-list { list-style: none; padding-left: 0; }
    
    .task-details-list li {
      padding: 0.4em 0;
      border-bottom: 1px solid #eee;
    }
    
    .line-edit {
      padding: 0.75em 1em;
      margin: 0.5em 0;
      background: #f8f8f6;
      border-left: 3px solid #8a6d00;
    }
    
    .line-edit-original {
      color: #8b2e2e;
      text-decoration: line-through;
      margin-bottom: 0.25em;
      font-size: 9pt;
    }
    
    .line-edit-suggested {
      color: #2d5a27;
      font-weight: 500;
      font-size: 9pt;
    }
    
    .line-edit-reason {
      margin-top: 0.35em;
      font-size: 8pt;
      color: #555;
      font-style: italic;
    }
    
    .section-desc {
      color: #555;
      font-size: 9.5pt;
      margin: -0.5em 0 1em 0;
      line-height: 1.5;
    }
    
    .chunk-status-badge {
      padding: 0.2em 0.5em;
      font-size: 7pt;
      font-weight: 600;
      text-transform: uppercase;
      letter-spacing: 0.05em;
      background: #e8e8e6;
      color: #555;
    }
    
    .chunk-status-badge.ok, .chunk-status-badge.passed {
      background: #e8f0e8;
      color: #2d5a27;
    }
    
    .more-edits, .more-chunks {
      text-align: center;
      color: #666;
      font-size: 9pt;
      margin-top: 0.75em;
    }
    
    .methodology-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
      gap: 1em;
      margin: 1.5em 0;
    }
    
    .methodology-item {
      text-align: center;
      padding: 1em;
      background: #f8f8f6;
      border: 1px solid #e0e0dc;
    }
    
    .methodology-label {
      font-size: 8pt;
      color: #555;
      margin-bottom: 0.35em;
      text-transform: uppercase;
      letter-spacing: 0.05em;
    }
    
    .methodology-score {
      font-size: 14pt;
      font-weight: 700;
      color: #1a1a1a;
    }
    
    table {
      width: 100%;
      border-collapse: collapse;
      margin: 1em 0;
      font-size: 9pt;
    }
    
    th, td {
      padding: 0.5em 0.75em;
      text-align: left;
      border: 1px solid #d4d4d4;
    }
    
    th {
      background: #f0f0ee;
      font-weight: 600;
      color: #1a1a1a;
      text-transform: uppercase;
      font-size: 8pt;
      letter-spacing: 0.05em;
    }
    
    .footer {
      text-align: center;
      margin-top: 2em;
      padding-top: 1.5em;
      border-top: 1pt solid #1a1a1a;
      color: #666;
      font-size: 8pt;
    }
    
    @media print {
      body { background: white; padding: 1cm; }
      .container { box-shadow: none; border: 1px solid #ccc; }
    }
  </style>
</head>
<body>
  <div class="container">
    <div class="header">
      <h1>Document Analysis Report</h1>
      <p class="subtitle">${manuscriptTitle || 'Manuscript Evaluation'}</p>
      <p class="meta">Report ID: ${reportData.report_id} | Generated: ${date}</p>
      ${reportData.journal_url ? `<p class="meta">Journal: <a href="${reportData.journal_url}" style="color: #1a1a1a; text-decoration: underline;">${reportData.journal_url}</a></p>` : ''}
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
      ${refereeData.methodology_rubric.notes ? `<p style="margin-top: 1em; color: #444; font-size: 9.5pt; line-height: 1.5;">${refereeData.methodology_rubric.notes}</p>` : ''}
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
          <li style="background: #f8f8f6; padding: 0.75em 1em; margin-bottom: 0.5em; border-left: 3px solid #8a6d00;">
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
          <li style="background: #f8f8f6; padding: 0.75em 1em; margin-bottom: 0.5em; border-left: 3px solid #2d5a27;">
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
              <td><span style="text-transform: uppercase; font-size: 8pt;">${item.evidence_type || ''}</span></td>
              <td><span style="padding: 0.2em 0.5em; background: ${item.strength === 'strong' ? '#e8f0e8' : '#f5f0e0'}; color: ${item.strength === 'strong' ? '#2d5a27' : '#8a6d00'}; font-size: 7pt; font-weight: 600;">${item.strength || ''}</span></td>
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
      <p class="section-desc">Line-by-line analysis by manuscript section. Each section shows AI use, plagiarism, guidelines, novelty, and suggested edits.</p>
      ${reportData.chunks.slice(0, 15).map((chunk: any, idx: number) => {
        const sectionName = generateSectionName(chunk, idx);
        const renderTaskDetails = (taskData: any) => {
          let detailsHtml = '';
          const details = taskData.details;
          if (details) {
            if (typeof details === 'string') {
              detailsHtml = `<div class="task-details">${escapeHtml(details)}</div>`;
            } else if (details.edits && Array.isArray(details.edits)) {
              detailsHtml = details.edits.slice(0, 5).map((e: any) => `
                <div class="line-edit">
                  <div class="line-edit-original">${escapeHtml((e.original_snippet || e.original || '').slice(0, 200))}${(e.original_snippet || e.original || '').length > 200 ? '...' : ''}</div>
                  <div class="line-edit-suggested">→ ${escapeHtml((e.suggested_snippet || e.suggested || '').slice(0, 200))}${(e.suggested_snippet || e.suggested || '').length > 200 ? '...' : ''}</div>
                  ${e.explanation ? `<div class="line-edit-reason">${escapeHtml(e.explanation)}</div>` : ''}
                </div>
              `).join('');
              if (details.edits.length > 5) detailsHtml += `<p class="more-edits">+ ${details.edits.length - 5} more edits</p>`;
              detailsHtml = `<div class="task-details line-edits">${detailsHtml}</div>`;
            } else if (details.failed_items && Array.isArray(details.failed_items)) {
              detailsHtml = `<ul class="task-details-list">${details.failed_items.map((i: any) => `<li>${escapeHtml(i.requirement || i.item || String(i))} ${i.suggested_fix ? `— Fix: ${escapeHtml(i.suggested_fix)}` : ''}</li>`).join('')}</ul>`;
            } else if (details.verbatim_matches && Array.isArray(details.verbatim_matches)) {
              detailsHtml = details.verbatim_matches.slice(0, 3).map((m: any) => `<div class="match-item">"${escapeHtml((m.phrase || '').slice(0, 80))}..." ${m.source ? `(${escapeHtml(m.source)})` : ''}</div>`).join('');
              detailsHtml = `<div class="task-details">${detailsHtml}</div>`;
            } else {
              detailsHtml = `<div class="task-details">${escapeHtml(JSON.stringify(details).slice(0, 300))}${JSON.stringify(details).length > 300 ? '...' : ''}</div>`;
            }
          }
          return detailsHtml;
        };
        return `
        <div class="chunk-card">
          <div class="chunk-header">
            <span class="chunk-id">${escapeHtml(sectionName)}</span>
            <span class="chunk-status-badge ${chunk.status || 'ok'}">${(chunk.status || 'ok').replace(/_/g, ' ')}</span>
          </div>
          ${chunk.task_results ? `
            <div class="task-grid">
              ${Object.entries(chunk.task_results).map(([taskName, taskData]: [string, any]) => `
                <div class="task-card">
                  <div class="task-title">${taskName.replace(/_/g, ' ').toUpperCase()}</div>
                  <div class="task-status ${taskData.task_status || 'ok'}">${(taskData.task_status || 'ok').replace(/_/g, ' ')}</div>
                  <div class="task-confidence">Confidence: ${formatConfidence(taskData.confidence || 0)}</div>
                  <div class="task-summary">${escapeHtml(taskData.summary || '')}</div>
                  ${renderTaskDetails(taskData)}
                </div>
              `).join('')}
            </div>
          ` : ''}
        </div>
      `;
      }).join('')}
      ${reportData.chunks.length > 15 ? `<p class="more-chunks">... and ${reportData.chunks.length - 15} more sections</p>` : ''}
    </div>
    ` : ''}

    <div class="footer">
      <p>Document Analysis Report · ${date}</p>
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
  link.download = filename || `document-analysis-report-${reportData.report_id}.html`;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);
};
