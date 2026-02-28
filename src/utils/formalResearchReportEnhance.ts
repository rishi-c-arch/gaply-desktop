/**
 * Wraps or injects formal international research report styling and Chart.js
 * so reports have high-quality typography and figures (including auto-charts from tables).
 */

const FORMAL_RESEARCH_CSS = `
  .formal-research-report, .formal-research-report body { font-family: 'Times New Roman', Times, Georgia, serif; font-size: 12pt; line-height: 1.5; color: #1a1a1a; background: #fff; margin: 0; padding: 0; }
  .formal-research-report body { max-width: 21cm; margin: 0 auto; padding: 2.5cm 2cm; }
  .formal-research-report h1 { font-size: 16pt; font-weight: bold; text-align: center; margin: 24px 0 16px; border-bottom: none; page-break-after: avoid; }
  .formal-research-report h2 { font-size: 14pt; font-weight: bold; margin: 28px 0 12px; page-break-after: avoid; }
  .formal-research-report h3 { font-size: 12pt; font-weight: bold; margin: 20px 0 10px; page-break-after: avoid; }
  .formal-research-report p { margin: 0 0 10px; text-align: justify; hyphens: auto; }
  .formal-research-report table { width: 100%; border-collapse: collapse; margin: 16px 0; font-size: 11pt; page-break-inside: avoid; }
  .formal-research-report th, .formal-research-report td { border: 1px solid #333; padding: 8px 10px; text-align: left; }
  .formal-research-report th { background: #f5f5f5; font-weight: bold; }
  .formal-research-report caption, .formal-research-report .table-caption { font-size: 11pt; margin-top: 6px; font-style: italic; text-align: left; }
  .formal-research-report .figure-wrapper { margin: 24px 0; page-break-inside: avoid; width: 100%; }
  .formal-research-report .figure-wrapper canvas { display: block; max-width: 100%; width: 100% !important; height: 340px !important; }
  .formal-research-report .figure-caption { font-size: 11pt; font-style: italic; margin-top: 8px; }
  .formal-research-report .figure-note { font-size: 10pt; color: #555; margin-top: 4px; }
  .formal-research-report ul, .formal-research-report ol { margin: 10px 0; padding-left: 24px; }
  .formal-research-report li { margin: 4px 0; }
  .formal-research-report .report-meta { font-size: 10pt; color: #555; margin-bottom: 20px; padding-bottom: 12px; border-bottom: 1px solid #ddd; }
  @media print { .formal-research-report body { padding: 1.5cm; } .formal-research-report .figure-wrapper, .formal-research-report table { page-break-inside: avoid; } }
`;

const CHART_JS_CDN = '<script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.1/dist/chart.umd.min.js"><\u002fscript>';

const CHART_SCRIPT = `
<script>
(function() {
  if (typeof Chart === 'undefined') return;
  function textContent(el) { return el ? (el.textContent || '').trim() : ''; }
  function parseNum(v) { var n = parseFloat(String(v).replace(/[^0-9.-]/g, '')); return isNaN(n) ? 0 : n; }
  function getTableData(tbl) {
    var thead = tbl.querySelector('thead');
    var tbody = tbl.querySelector('tbody') || tbl;
    var rows = tbody.querySelectorAll('tr');
    if (rows.length < 2) return null;
    var headerCells = thead ? thead.querySelectorAll('th') : (rows[0].querySelectorAll('td, th'));
    var dataRows = thead ? Array.from(rows) : Array.from(rows).slice(1);
    if (headerCells.length < 2 || dataRows.length === 0) return null;
    var labels = [];
    var values = [];
    var valueCol = 1;
    var headerText = headerCells[1] ? textContent(headerCells[1]).toLowerCase() : '';
    if (headerText.indexOf('odds') !== -1 || headerText.indexOf('coefficient') !== -1 || headerText.indexOf('or') !== -1) valueCol = 1;
    else if (headerCells.length > 2 && (textContent(headerCells[2]).toLowerCase().indexOf('ci') === -1)) valueCol = 2;
    dataRows.forEach(function(row) {
      var cells = row.querySelectorAll('td, th');
      if (cells.length > valueCol) {
        labels.push(textContent(cells[0]).slice(0, 35));
        values.push(parseNum(textContent(cells[valueCol])));
      }
    });
    if (labels.length === 0) return null;
    return { labels: labels, values: values, valueLabel: textContent(headerCells[valueCol]) || 'Value' };
  }
  function renderBarChart(canvas, data, color) {
    var hasNeg = data.values.some(function(v){ return v < 0; });
    var colors = ['rgba(70,130,180,0.85)','rgba(72,187,120,0.85)','rgba(251,146,60,0.85)','rgba(239,68,68,0.85)','rgba(139,92,246,0.85)','rgba(236,72,153,0.85)'];
    var bg = data.values.length <= 6 ? data.values.map(function(_, i){ return colors[i % colors.length]; }) : 'rgba(70,130,180,0.75)';
    new Chart(canvas, {
      type: 'bar',
      data: {
        labels: data.labels,
        datasets: [{ label: data.valueLabel, data: data.values, backgroundColor: bg, borderColor: bg.length ? bg.map(function(c){ return c.replace('0.85','1').replace('0.75','1'); }) : 'rgb(70,130,180)', borderWidth: 1 }]
      },
      options: {
        responsive: true,
        maintainAspectRatio: true,
        plugins: { legend: { display: false }, title: { display: false } },
        scales: { y: { beginAtZero: !hasNeg, ticks: { maxTicksLimit: 8 } }, x: { ticks: { maxRotation: 45, minRotation: 0, font: { size: 11 } } } }
      }
    });
  }
  var root = document.querySelector('.formal-research-report');
  if (!root) return;
  var processedTables = new Set();
  [].forEach.call(root.querySelectorAll('p, div, figcaption, span'), function(node) {
    var txt = textContent(node);
    if (!/Figure\\s*\\d+\\s*:/i.test(txt) || txt.length < 10) return;
    var childCap = node.querySelector && node.querySelector('p, figcaption');
    if (childCap && /Figure\\s*\\d+\\s*:/i.test(textContent(childCap))) return;
    var next = node.nextElementSibling;
    if (next && next.querySelector && next.querySelector('canvas')) return;
    var prev = node.previousElementSibling;
    var tbl = null;
    while (prev) {
      if (prev.tagName === 'TABLE') { tbl = prev; break; }
      if (prev.querySelector) { var q = prev.querySelector('table'); if (q) { tbl = q; break; } }
      prev = prev.previousElementSibling;
    }
    if (!tbl || processedTables.has(tbl)) return;
    var data = getTableData(tbl);
    if (!data || data.values.every(function(v){ return v === 0; })) return;
    processedTables.add(tbl);
    var wrapper = document.createElement('div');
    wrapper.className = 'figure-wrapper';
    var canvas = document.createElement('canvas');
    canvas.height = 340;
    canvas.width = 640;
    wrapper.appendChild(canvas);
    if (node.parentNode) {
      node.parentNode.insertBefore(wrapper, node.nextSibling);
          renderBarChart(canvas, data);
    }
  });
  root.querySelectorAll('table').forEach(function(tbl, idx) {
    if (processedTables.has(tbl)) return;
    var data = getTableData(tbl);
    if (!data || data.values.every(function(v){ return v === 0; })) return;
    var wrapper = document.createElement('div');
    wrapper.className = 'figure-wrapper';
    var caption = document.createElement('p');
    caption.className = 'figure-caption';
    caption.textContent = 'Figure ' + (idx + 1) + '. ' + data.valueLabel;
    var canvas = document.createElement('canvas');
    canvas.height = 320;
    wrapper.appendChild(canvas);
    wrapper.appendChild(caption);
    tbl.parentNode.insertBefore(wrapper, tbl);
    renderBarChart(canvas, data);
  });
})();
<\u002fscript>`;

/**
 * Injects formal research styling and Chart.js into report HTML.
 * If the HTML has <head> and <body>, injects into them; otherwise wraps in a full document.
 */
export function enhanceFormalReportHTML(rawHtml: string, reportTitle?: string): string {
  if (!rawHtml || typeof rawHtml !== 'string') return rawHtml;

  const hasStructure = /<\s*html|<\s*head|<\s*body/i.test(rawHtml);

  if (hasStructure) {
    let out = rawHtml;

    // Inject CSS before </head>
    if (/<\s*\/\s*head\s*>/i.test(out)) {
      out = out.replace(/<\s*\/\s*head\s*>/i, '<style>' + FORMAL_RESEARCH_CSS + '</style>' + CHART_JS_CDN + '</head>');
    } else {
      out = out.replace(/<\s*head\s*>/i, '<head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>' + (reportTitle || 'Research Report') + '</title><style>' + FORMAL_RESEARCH_CSS + '</style>' + CHART_JS_CDN);
    }

    // Add class to body for formal styling
    if (!/formal-research-report/i.test(out)) {
      out = out.replace(/<\s*body(\s[^>]*)?>/i, function(match) {
        if (/class\s*=/i.test(match)) return match.replace(/\bclass\s*=\s*["']([^"']*)["']/, 'class="$1 formal-research-report"');
        return match.replace(/<\s*body/i, '<body class="formal-research-report"');
      });
    }

    // Inject chart script before </body>
    if (/<\s*\/\s*body\s*>/i.test(out)) {
      out = out.replace(/<\s*\/\s*body\s*>/i, CHART_SCRIPT + '</body>');
    }

    return out;
  }

  // No structure: wrap in full document
  const title = reportTitle || 'Statistical Research Report';
  return '<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>' + escapeHtml(title) + '</title><style>' + FORMAL_RESEARCH_CSS + '</style>' + CHART_JS_CDN + '</head><body class="formal-research-report">' + rawHtml + CHART_SCRIPT + '</body></html>';
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}
