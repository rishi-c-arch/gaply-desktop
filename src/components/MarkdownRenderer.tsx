import React from 'react';
import './MarkdownRenderer.css';

interface MarkdownRendererProps {
  content: string;
}

const MarkdownRenderer: React.FC<MarkdownRendererProps> = ({ content }) => {
  const lines = content.split('\n');
  const elements: React.ReactNode[] = [];
  let currentList: string[] = [];
  let listType: 'ul' | 'ol' | null = null;
  let inCodeBlock = false;
  let codeBlockContent: string[] = [];
  let currentTable: string[] = [];
  let tableStarted = false;

  const flushList = () => {
    if (currentList.length > 0 && listType) {
      if (listType === 'ul') {
        elements.push(
          <ul key={`list-${elements.length}`} className="md-list md-list-unordered">
            {currentList.map((item, idx) => (
              <li key={idx} className="md-list-item">{renderInlineMarkdown(item.trim())}</li>
            ))}
          </ul>
        );
      } else {
        elements.push(
          <ol key={`list-${elements.length}`} className="md-list md-list-ordered">
            {currentList.map((item, idx) => (
              <li key={idx} className="md-list-item">{renderInlineMarkdown(item.replace(/^\d+\.\s*/, ''))}</li>
            ))}
          </ol>
        );
      }
      currentList = [];
      listType = null;
    }
  };

  const flushTable = () => {
    if (currentTable.length > 0 && tableStarted) {
      elements.push(
        <div key={`table-${elements.length}`} className="md-table-container">
          {currentTable.map((row, idx) => {
            const cells = row.split('|').map(c => c.trim()).filter(c => c);
            return (
              <div key={idx} className={`md-table-row ${idx === 0 ? 'md-table-header' : ''}`}>
                {cells.map((cell, cellIdx) => (
                  <div key={cellIdx} className="md-table-cell">{renderInlineMarkdown(cell)}</div>
                ))}
              </div>
            );
          })}
        </div>
      );
      currentTable = [];
      tableStarted = false;
    }
  };

  const renderInlineMarkdown = (text: string): React.ReactNode => {
    // Handle LaTeX: $$...$$ block first, then $...$ inline (formulas from AI)
    text = text.replace(/\$\$([^$]+)\$\$/g, '<div class="md-formula-block"><code>$1</code></div>');
    text = text.replace(/\$([^$]+)\$/g, '<code class="md-inline-code md-formula">$1</code>');
    // Handle bold **text**
    text = text.replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>');
    // Handle italic *text* (simplified - avoid conflicts)
    text = text.replace(/(?<!\*)\*(?!\*)([^*]+?)(?<!\*)\*(?!\*)/g, '<em>$1</em>');
    // Handle code `code`
    text = text.replace(/`([^`]+)`/g, '<code class="md-inline-code">$1</code>');
    
    return <span dangerouslySetInnerHTML={{ __html: text }} />;
  };

  lines.forEach((line, index) => {
    const trimmed = line.trim();

    // Code blocks
    if (trimmed.startsWith('```')) {
      if (inCodeBlock) {
        // End code block
        elements.push(
          <pre key={`code-${elements.length}`} className="md-code-block">
            <code>{codeBlockContent.join('\n')}</code>
          </pre>
        );
        codeBlockContent = [];
        inCodeBlock = false;
      } else {
        // Start code block
        flushList();
        inCodeBlock = true;
      }
      return;
    }

    if (inCodeBlock) {
      codeBlockContent.push(line);
      return;
    }

    // Headers
    if (trimmed.startsWith('### ')) {
      flushList();
      flushTable();
      elements.push(
        <h3 key={`h3-${index}`} className="md-h3">{trimmed.replace(/^###\s+/, '')}</h3>
      );
      return;
    }

    if (trimmed.startsWith('#### ')) {
      flushList();
      flushTable();
      elements.push(
        <h4 key={`h4-${index}`} className="md-h4">{trimmed.replace(/^####\s+/, '')}</h4>
      );
      return;
    }

    if (trimmed.startsWith('## ')) {
      flushList();
      flushTable();
      elements.push(
        <h2 key={`h2-${index}`} className="md-h2">{trimmed.replace(/^##\s+/, '')}</h2>
      );
      return;
    }

    if (trimmed.startsWith('# ')) {
      flushList();
      flushTable();
      elements.push(
        <h1 key={`h1-${index}`} className="md-h1">{trimmed.replace(/^#\s+/, '')}</h1>
      );
      return;
    }

    // Unordered lists
    if (trimmed.match(/^[-*]\s+/)) {
      if (listType !== 'ul') {
        flushList();
        listType = 'ul';
      }
      currentList.push(trimmed.replace(/^[-*]\s+/, ''));
      return;
    }

    // Ordered lists
    if (trimmed.match(/^\d+\.\s+/)) {
      if (listType !== 'ol') {
        flushList();
        listType = 'ol';
      }
      currentList.push(trimmed);
      return;
    }

      // Tables (basic markdown table parsing)
      if (trimmed.includes('|') && trimmed.split('|').length > 2) {
        const isSeparator = trimmed.match(/^[\s|:|-]+$/);
        if (isSeparator) {
          // Separator line - table continues
          return;
        }
        
        flushList();
        if (!tableStarted) {
          tableStarted = true;
        }
        currentTable.push(trimmed);
        return;
      } else if (tableStarted) {
        // End of table
        flushTable();
      }

      // Regular paragraphs
      flushList();
      flushTable();
    if (trimmed) {
      elements.push(
        <p key={`p-${index}`} className="md-paragraph">{renderInlineMarkdown(trimmed)}</p>
      );
    } else if (elements.length > 0) {
      // Empty line after content - add spacing
      elements.push(<br key={`br-${index}`} />);
    }
  });

  flushList();
  flushTable();

  return <div className="md-content">{elements}</div>;
};

export default MarkdownRenderer;
