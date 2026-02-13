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

interface Annotation {
  id: string;
  start: number;
  end: number;
  text: string;
  type: 'guideline' | 'plagiarism' | 'ai_detection' | 'novelty' | 'edit' | 'reference';
  severity: 'critical' | 'major' | 'minor' | 'info';
  comment: string;
  suggestion?: string;
  confidence?: number;
}

export const generateTurnitinStyleReport = (
  reportData: ReportData,
  manuscriptText: string
): string => {
  const date = new Date().toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit'
  });

  // Extract all annotations from chunks and analysis
  const annotations: Annotation[] = [];
  let annotationId = 1;

  // Calculate chunk positions in full text (fallback if exact match fails)
  let cumulativePos = 0;
  const chunkPositions: number[] = [];
  
  // Parse chunks for edits and issues
  if (reportData.chunks && reportData.chunks.length > 0) {
    reportData.chunks.forEach((chunk, chunkIdx) => {
      chunkPositions.push(cumulativePos);
      const chunkText = chunk.chunk_text || chunk.text || '';
      if (chunkText) {
        // Try to find exact match first
        let chunkStartInFullText = manuscriptText.indexOf(chunkText);
        // If not found, use cumulative position
        if (chunkStartInFullText < 0) {
          chunkStartInFullText = cumulativePos;
        }
        cumulativePos = chunkStartInFullText + chunkText.length;
      } else {
        cumulativePos += 500; // Estimate chunk size
      }

      // Parse task_results if it's a string
      let taskResults = chunk.task_results;
      if (typeof taskResults === 'string') {
        try {
          taskResults = JSON.parse(taskResults);
        } catch (e) {
          console.warn('Failed to parse task_results:', e);
          taskResults = null;
        }
      }

      if (taskResults) {
        const chunkText = chunk.chunk_text || chunk.text || '';
        const chunkStartPos = chunkPositions[chunkIdx] || 0;
        
        // Extract suggest_edits
        const suggestEdits = taskResults.suggest_edits;
        if (suggestEdits) {
          let details = suggestEdits.details;
          // Parse details if it's a string
          if (typeof details === 'string') {
            try {
              details = JSON.parse(details);
            } catch (e) {
              details = null;
            }
          }

          if (details?.edits && Array.isArray(details.edits)) {
            details.edits.forEach((edit: any) => {
              if (edit.location && Array.isArray(edit.location) && edit.location.length >= 2) {
                const [startOffset, endOffset] = edit.location;
                const absoluteStart = chunkStartPos + Math.max(0, startOffset);
                const absoluteEnd = chunkStartPos + Math.min(chunkText.length, endOffset || absoluteStart + 50);

                annotations.push({
                  id: `edit-${annotationId++}`,
                  start: Math.max(0, absoluteStart),
                  end: Math.min(manuscriptText.length, absoluteEnd),
                  text: edit.original_snippet || chunkText.slice(startOffset, endOffset) || '',
                  type: 'edit',
                  severity: edit.type === 'critical' ? 'critical' : edit.type === 'major' ? 'major' : 'minor',
                  comment: edit.explanation || suggestEdits.summary || 'Suggested edit',
                  suggestion: edit.suggested_snippet || '',
                  confidence: suggestEdits.confidence || 0.8
                });
              } else if (edit.original_snippet || edit.suggested_snippet) {
                // Fallback: create annotation for chunk if we have edit info but no location
                const chunkMid = chunkStartPos + (chunkText.length / 2);
                annotations.push({
                  id: `edit-${annotationId++}`,
                  start: Math.max(0, chunkMid - 30),
                  end: Math.min(manuscriptText.length, chunkMid + 30),
                  text: edit.original_snippet || chunkText.slice(0, 60) || '',
                  type: 'edit',
                  severity: edit.type === 'critical' ? 'critical' : edit.type === 'major' ? 'major' : 'minor',
                  comment: edit.explanation || suggestEdits.summary || 'Suggested edit',
                  suggestion: edit.suggested_snippet || '',
                  confidence: suggestEdits.confidence || 0.8
                });
              }
            });
          } else if (suggestEdits.summary && suggestEdits.task_status !== 'passed') {
            // Fallback: if we have a summary but no edits array, create a general annotation
            const chunkMid = chunkStartPos + (chunkText.length / 2);
            annotations.push({
              id: `edit-${annotationId++}`,
              start: Math.max(0, chunkMid - 50),
              end: Math.min(manuscriptText.length, chunkMid + 50),
              text: chunkText.slice(0, 100) || '',
              type: 'edit',
              severity: 'minor',
              comment: suggestEdits.summary,
              confidence: suggestEdits.confidence || 0.7
            });
          }
        }

        // Extract plagiarism matches
        const plagiarismCheck = taskResults.plagiarism_check;
        if (plagiarismCheck && plagiarismCheck.task_status !== 'passed') {
          let plagDetails = plagiarismCheck.details;
          if (typeof plagDetails === 'string') {
            try {
              plagDetails = JSON.parse(plagDetails);
            } catch (e) {
              plagDetails = null;
            }
          }

          if (plagDetails?.verbatim_matches && Array.isArray(plagDetails.verbatim_matches)) {
            plagDetails.verbatim_matches.forEach((match: any) => {
              if (match.offset && Array.isArray(match.offset)) {
                const [start, end] = match.offset;
                const absoluteStart = chunkStartPos + Math.max(0, start || 0);
                const absoluteEnd = chunkStartPos + Math.min(chunkText.length, end || absoluteStart + 50);

                annotations.push({
                  id: `plag-${annotationId++}`,
                  start: Math.max(0, absoluteStart),
                  end: Math.min(manuscriptText.length, absoluteEnd),
                  text: match.phrase || chunkText.slice(start, end) || '',
                  type: 'plagiarism',
                  severity: (match.similarity || 0) > 0.8 ? 'critical' : (match.similarity || 0) > 0.6 ? 'major' : 'minor',
                  comment: `<strong>Plagiarism Detected:</strong> ${Math.round((match.similarity || 0) * 100)}% similarity with "${match.source || 'Unknown'}". ` +
                         `This text closely matches external sources and may require proper citation or paraphrasing.`,
                  confidence: match.similarity || plagiarismCheck.confidence || 0
                });
              }
            });
          } else if (plagiarismCheck.summary && plagiarismCheck.task_status === 'flagged') {
            // Fallback annotation for flagged plagiarism
            const chunkMid = chunkStartPos + (chunkText.length / 2);
            annotations.push({
              id: `plag-${annotationId++}`,
              start: Math.max(0, chunkMid - 50),
              end: Math.min(manuscriptText.length, chunkMid + 50),
              text: chunkText.slice(0, 100) || '',
              type: 'plagiarism',
              severity: 'major',
              comment: plagiarismCheck.summary,
              confidence: plagiarismCheck.confidence || 0.6
            });
          }
        }

        // Extract AI detection
        const aiDetection = taskResults.ai_use_detection;
        if (aiDetection && aiDetection.confidence && aiDetection.confidence > 0.5) {
          const aiConfidence = aiDetection.confidence || 0;
          const chunkMid = chunkStartPos + (chunkText.length / 2);
          
          let aiDetails = aiDetection.details;
          if (typeof aiDetails === 'string') {
            try {
              aiDetails = JSON.parse(aiDetails);
            } catch (e) {
              aiDetails = null;
            }
          }

          const recommendation = aiDetails?.recommendation || '';
          annotations.push({
            id: `ai-${annotationId++}`,
            start: Math.max(0, chunkMid - 50),
            end: Math.min(manuscriptText.length, chunkMid + 50),
            text: chunkText.slice(0, 100) || '',
            type: 'ai_detection',
            severity: aiConfidence > 0.8 ? 'major' : 'minor',
            comment: `<strong>AI-Generated Content Detected:</strong> This section shows ${Math.round(aiConfidence * 100)}% likelihood of AI-generated text. ` +
                     (recommendation ? `Recommendation: ${recommendation}` : 'Consider rewriting in a more natural, academic style.'),
            confidence: aiConfidence
          });
        }

        // Extract guideline violations
        const guidelineCheck = taskResults.guideline_check;
        if (guidelineCheck && guidelineCheck.task_status !== 'passed') {
          let guideDetails = guidelineCheck.details;
          if (typeof guideDetails === 'string') {
            try {
              guideDetails = JSON.parse(guideDetails);
            } catch (e) {
              guideDetails = null;
            }
          }

          if (guideDetails?.failed_items && Array.isArray(guideDetails.failed_items)) {
            guideDetails.failed_items.forEach((item: any) => {
              const chunkMid = chunkStartPos + (chunkText.length / 2);
              annotations.push({
                id: `guide-${annotationId++}`,
                start: Math.max(0, chunkMid - 50),
                end: Math.min(manuscriptText.length, chunkMid + 50),
                text: chunkText.slice(0, 100) || '',
                type: 'guideline',
                severity: 'minor',
                comment: `<strong>Guideline Violation:</strong> ${item.requirement || 'Does not meet journal guidelines'}. ` +
                       `<strong>Fix:</strong> ${item.suggested_fix || 'Please review journal submission requirements.'}`,
                suggestion: item.suggested_fix || ''
              });
            });
          } else if (guidelineCheck.summary && guidelineCheck.task_status === 'flagged') {
            // Fallback for flagged guidelines
            const chunkMid = chunkStartPos + (chunkText.length / 2);
            annotations.push({
              id: `guide-${annotationId++}`,
              start: Math.max(0, chunkMid - 50),
              end: Math.min(manuscriptText.length, chunkMid + 50),
              text: chunkText.slice(0, 100) || '',
              type: 'guideline',
              severity: 'minor',
              comment: guidelineCheck.summary,
              confidence: guidelineCheck.confidence || 0.7
            });
          }
        }
      }
    });
  }

  // Sort annotations by start position
  annotations.sort((a, b) => a.start - b.start);

  // Remove overlapping annotations (keep first one)
  const filteredAnnotations: Annotation[] = [];
  let lastEnd = -1;
  annotations.forEach(ann => {
    if (ann.start >= lastEnd) {
      filteredAnnotations.push(ann);
      lastEnd = ann.end;
    }
  });

  // HTML escape helper
  const escapeHtml = (text: string): string => {
    if (!text) return '';
    return String(text)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#039;')
      .replace(/\n/g, '<br>');
  };

  // Parse and convert markdown-style tables to HTML
  const parseTable = (text: string): { isTable: boolean; html: string; endPos: number } => {
    const lines = text.split('\n');
    if (lines.length < 2) return { isTable: false, html: '', endPos: 0 };
    
    // Check if current line looks like a table (has | or tabs)
    const firstLine = lines[0].trim();
    if (!firstLine.includes('|') && !firstLine.includes('\t') && !/^\s+[^\s]+\s+/.test(firstLine)) {
      return { isTable: false, html: '', endPos: 0 };
    }

    // Collect table rows (until we hit a non-table line or empty line)
    const tableRows: string[] = [];
    let lineIdx = 0;
    
    while (lineIdx < lines.length) {
      const line = lines[lineIdx].trim();
      // Skip separator lines like |---|---|
      if (line.match(/^[\s|:-]+$/)) {
        lineIdx++;
        continue;
      }
      
      // If line has | or tabs, it's part of table
      if (line.includes('|') || line.includes('\t') || /^\s+[^\s]+\s+/.test(line)) {
        tableRows.push(line);
        lineIdx++;
      } else {
        break;
      }
      
      if (tableRows.length >= 50) break; // Limit table size
    }

    if (tableRows.length < 2) {
      return { isTable: false, html: '', endPos: 0 };
    }

    // Convert to HTML table
    let tableHtml = '<table class="manuscript-table">';
    
    tableRows.forEach((row, idx) => {
      let cells: string[] = [];
      
      if (row.includes('|')) {
        cells = row.split('|').map(c => c.trim()).filter(c => c);
      } else if (row.includes('\t')) {
        cells = row.split('\t').map(c => c.trim()).filter(c => c);
      } else {
        // Space-separated (at least 2 spaces)
        cells = row.split(/\s{2,}/).map(c => c.trim()).filter(c => c);
      }

      if (cells.length === 0) return;
      
      const tag = idx === 0 ? 'th' : 'td';
      tableHtml += '<tr>';
      cells.forEach(cell => {
        tableHtml += `<${tag}>${escapeHtml(cell)}</${tag}>`;
      });
      tableHtml += '</tr>';
    });

    tableHtml += '</table>';
    const endPos = lines.slice(0, lineIdx).join('\n').length + (lineIdx > 0 ? 1 : 0);
    
    return { isTable: true, html: tableHtml, endPos };
  };

  // Detect if a line is a heading (all caps, numbered, or specific patterns)
  const isHeading = (line: string, prevLine: string = ''): { level: number; isHeading: boolean } => {
    const trimmed = line.trim();
    if (!trimmed) return { level: 0, isHeading: false };
    
    // Very short lines are likely not headings
    if (trimmed.length < 3) return { level: 0, isHeading: false };
    
    // All caps and reasonable length (likely main heading)
    if (trimmed === trimmed.toUpperCase() && trimmed.length > 5 && trimmed.length < 150 && /[A-Z]/.test(trimmed)) {
      return { level: 1, isHeading: true };
    }
    
    // Numbered headings (1., 1.1, 1.1.1, I., A., etc.)
    const numberedPattern = /^(\d+(\.\d+)*\.?\s+|[IVX]+\.\s+|[A-Z]\.\s+)/i;
    if (numberedPattern.test(trimmed)) {
      // Count decimal points to determine level (1. = level 2, 1.1 = level 3, etc.)
      const decimalCount = (trimmed.match(/\./g) || []).length;
      const level = decimalCount === 1 ? 2 : decimalCount >= 2 ? 3 : trimmed.match(/^[IVX]+\./i) ? 2 : 3;
      return { level, isHeading: true };
    }
    
    // Section headers (short lines after empty lines, often capitalized first letter of each word)
    if (prevLine.trim() === '' && trimmed.length < 80) {
      const words = trimmed.split(/\s+/);
      const capitalizedWords = words.filter(w => /^[A-Z][a-z]+/.test(w)).length;
      // If most words start with capital, likely a heading
      if (capitalizedWords >= words.length * 0.7 && words.length >= 2) {
        return { level: 2, isHeading: true };
      }
    }
    
    // Lines ending with colon might be headings
    if (trimmed.endsWith(':') && trimmed.length < 100 && !trimmed.includes('.')) {
      return { level: 3, isHeading: true };
    }
    
    // Bold-like patterns (if manuscript uses markdown or formatting hints)
    if (trimmed.startsWith('**') && trimmed.endsWith('**')) {
      return { level: 2, isHeading: true };
    }
    
    return { level: 0, isHeading: false };
  };

  // Generate highlighted manuscript HTML with proper heading structure
  const generateHighlightedManuscript = (): string => {
    if (!manuscriptText) return '<p style="color: #6e6e73; padding: 20px;">Manuscript text not available.</p>';

    let html = '<div class="manuscript-content">';
    let currentPos = 0;
    const processedAnnotations = [...filteredAnnotations];
    
    // Process text line by line to detect headings, tables, and annotations
    const lines = manuscriptText.split('\n');
    let linePos = 0;
    let prevLine = '';
    
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];
      const lineStart = linePos;
      const lineEnd = linePos + line.length;
      const trimmedLine = line.trim();
      
      // Check if this might be a table start
      const remainingText = lines.slice(i).join('\n');
      const tableResult = parseTable(remainingText);
      
      if (tableResult.isTable && tableResult.endPos > 0) {
        // Add annotations before table
        while (processedAnnotations.length > 0 && processedAnnotations[0].start < lineStart) {
          const ann = processedAnnotations.shift()!;
          if (currentPos < ann.start) {
            html += escapeHtml(manuscriptText.slice(currentPos, ann.start));
          }
          const annotationText = manuscriptText.slice(ann.start, ann.end);
          const highlightClass = `highlight-${ann.type} highlight-${ann.severity}`;
          html += `<span class="highlight ${highlightClass}" data-annotation-id="${ann.id}" onclick="showAnnotation('${ann.id}')">${escapeHtml(annotationText)}</span>`;
          currentPos = ann.end;
        }
        
        // Add table
        html += tableResult.html;
        const tableEndPos = lineStart + tableResult.endPos;
        currentPos = tableEndPos;
        linePos = tableEndPos;
        i += tableResult.endPos.toString().split('\n').length - 1;
        prevLine = '';
        continue;
      }

      // Check if this is a heading
      const headingInfo = isHeading(trimmedLine, prevLine);
      
      // Process annotations before this line
      while (processedAnnotations.length > 0 && processedAnnotations[0].start < lineEnd) {
        const ann = processedAnnotations[0];
        
        // Add text before annotation
        if (currentPos < ann.start) {
          html += escapeHtml(manuscriptText.slice(currentPos, ann.start));
        }

        // Add highlighted annotation
        const highlightClass = `highlight-${ann.type} highlight-${ann.severity}`;
        const annotationText = manuscriptText.slice(ann.start, ann.end);
        html += `<span class="highlight ${highlightClass}" data-annotation-id="${ann.id}" onclick="showAnnotation('${ann.id}')">${escapeHtml(annotationText)}</span>`;
        
        currentPos = ann.end;
        processedAnnotations.shift();
      }
      
      // Format line based on type
      if (headingInfo.isHeading) {
        // Add any remaining text before the heading
        if (currentPos < lineStart) {
          html += escapeHtml(manuscriptText.slice(currentPos, lineStart));
        }
        
        // Format heading based on level
        const headingClass = headingInfo.level === 1 ? 'manuscript-h1' : 
                           headingInfo.level === 2 ? 'manuscript-h2' : 'manuscript-h3';
        
        // Process annotations within heading text
        let headingHtml = '';
        let headingPos = lineStart;
        const headingAnnotations = processedAnnotations.filter(ann => ann.start >= lineStart && ann.end <= lineEnd);
        
        // Process heading with annotations
        headingAnnotations.forEach(ann => {
          if (headingPos < ann.start) {
            headingHtml += escapeHtml(manuscriptText.slice(headingPos, ann.start));
          }
          const highlightClass = `highlight-${ann.type} highlight-${ann.severity}`;
          const annotationText = manuscriptText.slice(ann.start, ann.end);
          headingHtml += `<span class="highlight ${highlightClass}" data-annotation-id="${ann.id}" onclick="showAnnotation('${ann.id}')">${escapeHtml(annotationText)}</span>`;
          headingPos = ann.end;
        });
        
        if (headingPos < lineEnd) {
          headingHtml += escapeHtml(manuscriptText.slice(headingPos, lineEnd));
        } else if (!headingHtml) {
          // No annotations, just escape the full heading text
          headingHtml = escapeHtml(trimmedLine);
        }
        
        html += `<h${headingInfo.level} class="${headingClass}">${headingHtml}</h${headingInfo.level}>`;
        currentPos = lineEnd;
      } else if (trimmedLine) {
        // Regular paragraph line
        if (currentPos < lineStart) {
          html += escapeHtml(manuscriptText.slice(currentPos, lineStart));
        }
        
        // Process annotations in this line
        const lineAnnotations = processedAnnotations.filter(ann => ann.start >= lineStart && ann.end <= lineEnd);
        let lineHtml = '';
        let linePos_local = lineStart;
        
        lineAnnotations.forEach(ann => {
          if (linePos_local < ann.start) {
            lineHtml += escapeHtml(manuscriptText.slice(linePos_local, ann.start));
          }
          const highlightClass = `highlight-${ann.type} highlight-${ann.severity}`;
          const annotationText = manuscriptText.slice(ann.start, ann.end);
          lineHtml += `<span class="highlight ${highlightClass}" data-annotation-id="${ann.id}" onclick="showAnnotation('${ann.id}')">${escapeHtml(annotationText)}</span>`;
          linePos_local = ann.end;
        });
        
        if (linePos_local < lineEnd) {
          lineHtml += escapeHtml(manuscriptText.slice(linePos_local, lineEnd));
        }
        
        // Wrap in paragraph if previous line was empty or was a heading
        if (prevLine.trim() === '' || isHeading(prevLine.trim()).isHeading) {
          html += `<p class="manuscript-paragraph">${lineHtml}</p>`;
        } else {
          html += lineHtml;
          html += '<br>';
        }
        
        currentPos = lineEnd;
      } else {
        // Empty line - add spacing
        if (currentPos < lineEnd) {
          html += escapeHtml(manuscriptText.slice(currentPos, lineEnd));
        }
        html += '<br>';
        currentPos = lineEnd;
      }
      
      prevLine = line;
      linePos = lineEnd + 1; // +1 for newline
    }

    // Add any remaining annotations
    processedAnnotations.forEach((ann) => {
      if (currentPos < ann.start) {
        html += escapeHtml(manuscriptText.slice(currentPos, ann.start));
      }
      const highlightClass = `highlight-${ann.type} highlight-${ann.severity}`;
      const annotationText = manuscriptText.slice(ann.start, ann.end);
      html += `<span class="highlight ${highlightClass}" data-annotation-id="${ann.id}" onclick="showAnnotation('${ann.id}')">${escapeHtml(annotationText)}</span>`;
      currentPos = ann.end;
    });

    // Add remaining text
    if (currentPos < manuscriptText.length) {
      html += escapeHtml(manuscriptText.slice(currentPos));
    }

    html += '</div>';
    return html;
  };

  // Generate sidebar with annotations
  const generateSidebar = (): string => {
    if (filteredAnnotations.length === 0) {
      return '<div class="sidebar-empty">No annotations found in this document.</div>';
    }

    return filteredAnnotations.map(ann => {
      const previewText = ann.text.length > 80 ? ann.text.slice(0, 80) + '...' : ann.text;
      return `
      <div class="annotation-item" id="annotation-${ann.id}">
        <div class="annotation-header">
          <span class="annotation-type-badge type-${ann.type}">${ann.type.replace(/_/g, ' ')}</span>
          <span class="annotation-severity-badge severity-${ann.severity}">${ann.severity}</span>
          ${ann.confidence ? `<span class="annotation-confidence">${Math.round(ann.confidence * 100)}%</span>` : ''}
        </div>
        <div class="annotation-text-preview">"${escapeHtml(previewText)}"</div>
        <div class="annotation-comment">${ann.comment}</div>
        ${ann.suggestion ? `<div class="annotation-suggestion"><strong>Suggestion:</strong> ${escapeHtml(ann.suggestion)}</div>` : ''}
      </div>
    `;
    }).join('');
  };

  const getTypeColor = (type: string) => {
    switch (type) {
      case 'edit': return '#007AFF';
      case 'plagiarism': return '#FF3B30';
      case 'ai_detection': return '#FF9500';
      case 'guideline': return '#34C759';
      case 'novelty': return '#5856D6';
      case 'reference': return '#AF52DE';
      default: return '#8E8E93';
    }
  };

  const html = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Turnitin-Style Report - ${reportData.report_id}</title>
  <style>
    * {
      margin: 0;
      padding: 0;
      box-sizing: border-box;
    }

    body {
      font-family: -apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif;
      background: #f5f5f7;
      color: #1d1d1f;
      overflow-x: hidden;
    }

    .header {
      background: linear-gradient(135deg, #1d1d1f 0%, #2d2d2f 100%);
      color: white;
      padding: 20px 40px;
      box-shadow: 0 2px 10px rgba(0,0,0,0.1);
      position: sticky;
      top: 0;
      z-index: 100;
    }

    .header-content {
      max-width: 1600px;
      margin: 0 auto;
      display: flex;
      justify-content: space-between;
      align-items: center;
    }

    .header h1 {
      font-size: 1.5rem;
      font-weight: 600;
      margin-bottom: 4px;
    }

    .header-meta {
      font-size: 0.85rem;
      color: rgba(255,255,255,0.7);
    }

    .summary-stats {
      display: flex;
      gap: 24px;
    }

    .stat-item {
      text-align: right;
    }

    .stat-label {
      font-size: 0.75rem;
      color: rgba(255,255,255,0.6);
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }

    .stat-value {
      font-size: 1.25rem;
      font-weight: 700;
      margin-top: 4px;
    }

    .main-container {
      display: flex;
      max-width: 1600px;
      margin: 0 auto;
      min-height: calc(100vh - 120px);
    }

    .manuscript-panel {
      flex: 1;
      background: white;
      margin: 20px;
      border-radius: 12px;
      box-shadow: 0 4px 20px rgba(0,0,0,0.08);
      overflow: hidden;
      display: flex;
      flex-direction: column;
    }

    .manuscript-header {
      padding: 20px 30px;
      border-bottom: 1px solid #e5e5e7;
      background: #fafafa;
    }

    .manuscript-header h2 {
      font-size: 1.1rem;
      font-weight: 600;
      margin-bottom: 4px;
    }

    .manuscript-scroll {
      flex: 1;
      overflow-y: auto;
      padding: 30px;
      line-height: 1.8;
      font-size: 15px;
    }

    .manuscript-content {
      max-width: 800px;
      margin: 0 auto;
      text-align: justify;
      font-family: 'Times New Roman', Times, serif;
      color: #1d1d1f;
    }

    .manuscript-h1 {
      font-size: 1.5rem;
      font-weight: 700;
      margin: 24px 0 12px 0;
      color: #1d1d1f;
      line-height: 1.3;
      text-transform: uppercase;
      letter-spacing: 0.5px;
      padding-bottom: 8px;
      border-bottom: 2px solid #e5e5e7;
    }

    .manuscript-h2 {
      font-size: 1.25rem;
      font-weight: 700;
      margin: 20px 0 10px 0;
      color: #1d1d1f;
      line-height: 1.4;
      font-style: italic;
    }

    .manuscript-h3 {
      font-size: 1.1rem;
      font-weight: 600;
      margin: 16px 0 8px 0;
      color: #1d1d1f;
      line-height: 1.4;
    }

    .manuscript-paragraph {
      margin: 12px 0;
      line-height: 1.8;
      text-align: justify;
      text-indent: 0.5in;
    }

    .manuscript-paragraph:first-of-type {
      text-indent: 0;
    }

    .manuscript-paragraph:first-of-type:after {
      content: '';
      display: block;
      margin-bottom: 12px;
    }

    .manuscript-table {
      width: 100%;
      border-collapse: collapse;
      margin: 20px 0;
      background: white;
      border: 1px solid #e5e5e7;
      border-radius: 8px;
      overflow: hidden;
      box-shadow: 0 2px 8px rgba(0,0,0,0.08);
    }

    .manuscript-table th {
      background: #f5f5f7;
      padding: 12px 16px;
      text-align: left;
      font-weight: 600;
      color: #1d1d1f;
      border-bottom: 2px solid #d2d2d7;
      font-size: 0.9rem;
    }

    .manuscript-table td {
      padding: 10px 16px;
      border-bottom: 1px solid #e5e5e7;
      color: #1d1d1f;
      line-height: 1.5;
    }

    .manuscript-table tr:hover {
      background: #fafafa;
    }

    .manuscript-table tr:last-child td {
      border-bottom: none;
    }

    .highlight {
      background-color: rgba(255, 235, 59, 0.4);
      padding: 2px 0;
      cursor: pointer;
      border-bottom: 2px solid;
      position: relative;
      transition: background-color 0.2s;
    }

    .highlight:hover {
      background-color: rgba(255, 235, 59, 0.6);
    }

    .highlight-edit {
      background-color: rgba(0, 122, 255, 0.3);
      border-bottom-color: #007AFF;
    }

    .highlight-plagiarism {
      background-color: rgba(255, 59, 48, 0.3);
      border-bottom-color: #FF3B30;
    }

    .highlight-ai_detection {
      background-color: rgba(255, 149, 0, 0.3);
      border-bottom-color: #FF9500;
    }

    .highlight-guideline {
      background-color: rgba(52, 199, 89, 0.3);
      border-bottom-color: #34C759;
    }

    .highlight-novelty {
      background-color: rgba(88, 86, 214, 0.3);
      border-bottom-color: #5856D6;
    }

    .highlight-reference {
      background-color: rgba(175, 82, 222, 0.3);
      border-bottom-color: #AF52DE;
    }

    .highlight-critical {
      border-bottom-width: 3px;
    }

    .highlight-major {
      border-bottom-width: 2.5px;
    }

    .sidebar {
      width: 380px;
      background: #fafafa;
      border-left: 1px solid #e5e5e7;
      overflow-y: auto;
      padding: 20px;
    }

    .sidebar-header {
      margin-bottom: 20px;
      padding-bottom: 16px;
      border-bottom: 2px solid #e5e5e7;
    }

    .sidebar-header h3 {
      font-size: 1.1rem;
      font-weight: 600;
      margin-bottom: 8px;
    }

    .sidebar-count {
      font-size: 0.85rem;
      color: #6e6e73;
    }

    .annotation-item {
      background: white;
      border-radius: 8px;
      padding: 16px;
      margin-bottom: 16px;
      box-shadow: 0 2px 8px rgba(0,0,0,0.06);
      border-left: 4px solid;
      transition: transform 0.2s, box-shadow 0.2s;
    }

    .annotation-item:hover {
      transform: translateX(-2px);
      box-shadow: 0 4px 12px rgba(0,0,0,0.1);
    }

    .annotation-item.type-edit {
      border-left-color: #007AFF;
    }

    .annotation-item.type-plagiarism {
      border-left-color: #FF3B30;
    }

    .annotation-item.type-ai_detection {
      border-left-color: #FF9500;
    }

    .annotation-item.type-guideline {
      border-left-color: #34C759;
    }

    .annotation-header {
      display: flex;
      gap: 8px;
      align-items: center;
      margin-bottom: 12px;
      flex-wrap: wrap;
    }

    .annotation-type-badge {
      padding: 4px 10px;
      border-radius: 12px;
      font-size: 0.75rem;
      font-weight: 600;
      text-transform: uppercase;
      letter-spacing: 0.5px;
      background: #f0f0f0;
      color: #1d1d1f;
    }

    .annotation-type-badge.type-edit {
      background: rgba(0, 122, 255, 0.15);
      color: #007AFF;
    }

    .annotation-type-badge.type-plagiarism {
      background: rgba(255, 59, 48, 0.15);
      color: #FF3B30;
    }

    .annotation-type-badge.type-ai_detection {
      background: rgba(255, 149, 0, 0.15);
      color: #FF9500;
    }

    .annotation-type-badge.type-guideline {
      background: rgba(52, 199, 89, 0.15);
      color: #34C759;
    }

    .annotation-severity-badge {
      padding: 4px 10px;
      border-radius: 12px;
      font-size: 0.75rem;
      font-weight: 600;
      text-transform: uppercase;
    }

    .annotation-severity-badge.severity-critical {
      background: rgba(255, 59, 48, 0.15);
      color: #FF3B30;
    }

    .annotation-severity-badge.severity-major {
      background: rgba(255, 149, 0, 0.15);
      color: #FF9500;
    }

    .annotation-severity-badge.severity-minor {
      background: rgba(52, 199, 89, 0.15);
      color: #34C759;
    }

    .annotation-confidence {
      margin-left: auto;
      font-size: 0.75rem;
      color: #6e6e73;
    }

    .annotation-text-preview {
      font-style: italic;
      color: #6e6e73;
      font-size: 0.9rem;
      margin-bottom: 12px;
      padding: 8px;
      background: #f5f5f7;
      border-radius: 4px;
    }

    .annotation-comment {
      color: #1d1d1f;
      line-height: 1.6;
      margin-bottom: 8px;
    }

    .annotation-suggestion {
      margin-top: 12px;
      padding: 12px;
      background: #e8f4fd;
      border-radius: 6px;
      border-left: 3px solid #007AFF;
      font-size: 0.9rem;
      line-height: 1.6;
    }

    .sidebar-empty {
      text-align: center;
      padding: 40px 20px;
      color: #6e6e73;
      font-style: italic;
    }

    .annotation-item.highlighted {
      background: #fff9e6;
      box-shadow: 0 4px 16px rgba(255, 235, 59, 0.4);
    }

    @media print {
      .sidebar {
        display: none;
      }

      .header {
        background: white;
        color: #1d1d1f;
      }

      .manuscript-panel {
        margin: 0;
        box-shadow: none;
      }
    }
  </style>
</head>
<body>
  <div class="header">
    <div class="header-content">
      <div>
        <h1>Document Analysis Report</h1>
        <div class="header-meta">Report ID: ${reportData.report_id} | Generated: ${date}</div>
      </div>
      <div class="summary-stats">
        <div class="stat-item">
          <div class="stat-label">Total Issues</div>
          <div class="stat-value">${filteredAnnotations.length}</div>
        </div>
        <div class="stat-item">
          <div class="stat-label">Critical</div>
          <div class="stat-value" style="color: #FF3B30;">${filteredAnnotations.filter(a => a.severity === 'critical').length}</div>
        </div>
        <div class="stat-item">
          <div class="stat-label">Major</div>
          <div class="stat-value" style="color: #FF9500;">${filteredAnnotations.filter(a => a.severity === 'major').length}</div>
        </div>
        <div class="stat-item">
          <div class="stat-label">Minor</div>
          <div class="stat-value" style="color: #34C759;">${filteredAnnotations.filter(a => a.severity === 'minor').length}</div>
        </div>
      </div>
    </div>
  </div>

  <div class="main-container">
    <div class="manuscript-panel">
      <div class="manuscript-header">
        <h2>Manuscript with Inline Annotations</h2>
        <p style="font-size: 0.85rem; color: #6e6e73; margin-top: 4px;">Click highlighted text to view detailed comments</p>
      </div>
      <div class="manuscript-scroll">
        ${generateHighlightedManuscript()}
      </div>
    </div>

    <div class="sidebar">
      <div class="sidebar-header">
        <h3>Annotations</h3>
        <div class="sidebar-count">${filteredAnnotations.length} issue${filteredAnnotations.length !== 1 ? 's' : ''} found</div>
      </div>
      ${generateSidebar()}
    </div>
  </div>

  <script>
    function escapeHtml(text) {
      const div = document.createElement('div');
      div.textContent = text;
      return div.innerHTML.replace(/\\n/g, '<br>');
    }

    function showAnnotation(id) {
      // Remove previous highlights
      document.querySelectorAll('.annotation-item').forEach(item => {
        item.classList.remove('highlighted');
      });

      // Highlight selected annotation
      const annotationEl = document.getElementById('annotation-' + id);
      if (annotationEl) {
        annotationEl.classList.add('highlighted');
        annotationEl.scrollIntoView({ behavior: 'smooth', block: 'center' });
      }

      // Highlight corresponding text
      document.querySelectorAll('.highlight').forEach(highlight => {
        highlight.style.backgroundColor = '';
        if (highlight.getAttribute('data-annotation-id') === id) {
          highlight.style.backgroundColor = 'rgba(255, 235, 59, 0.8)';
        }
      });
    }

    // Click handler for all highlights
    document.querySelectorAll('.highlight').forEach(highlight => {
      highlight.addEventListener('click', function() {
        const id = this.getAttribute('data-annotation-id');
        if (id) showAnnotation(id);
      });
    });
  </script>
</body>
</html>`;

  return html;
};

export const downloadTurnitinStyleReport = (
  reportData: ReportData,
  manuscriptText: string,
  filename?: string
) => {
  const html = generateTurnitinStyleReport(reportData, manuscriptText);
  const blob = new Blob([html], { type: 'text/html' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename || `turnitin-style-report-${reportData.report_id}.html`;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);
};
