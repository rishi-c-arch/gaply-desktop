/**
 * Formal international research report styling + auto-charts from every table.
 * Chart.js is loaded dynamically so it runs after load (fixes srcdoc/iframe).
 * Every table gets a chart; multiple chart types (bar, line, pie) for world-class output.
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
  .formal-research-report .figure-wrapper { margin: 28px 0; page-break-inside: avoid; width: 100%; position: relative; }
  .formal-research-report .figure-wrapper .chart-container { position: relative; width: 100%; min-height: 320px; max-height: 420px; }
  .formal-research-report .figure-wrapper canvas { display: block; max-width: 100%; width: 100% !important; height: 360px !important; border-radius: 4px; }
  .formal-research-report .figure-caption { font-size: 11pt; font-style: italic; margin-top: 10px; color: #333; }
  .formal-research-report .figure-note { font-size: 10pt; color: #555; margin-top: 4px; }
  .formal-research-report ul, .formal-research-report ol { margin: 10px 0; padding-left: 24px; }
  .formal-research-report li { margin: 4px 0; }
  .formal-research-report .report-meta { font-size: 10pt; color: #555; margin-bottom: 20px; padding-bottom: 12px; border-bottom: 1px solid #ddd; }
  @media print { .formal-research-report body { padding: 1.5cm; } .formal-research-report .figure-wrapper, .formal-research-report table { page-break-inside: avoid; } }
`;

// Load Chart.js dynamically and run chart injection in onload so Chart is always defined (fixes iframe/srcdoc)
const CHART_SCRIPT = `
<script>
(function() {
  function loadScript(src, done) {
    var s = document.createElement('script');
    s.src = src;
    s.onload = done;
    s.onerror = function() { setTimeout(done, 100); };
    (document.head || document.documentElement).appendChild(s);
  }
  function runCharts() {
    if (typeof Chart === 'undefined') return;
    var ChartLib = Chart;
    function textContent(el) { return el ? (el.textContent || '').trim() : ''; }
    function parseNum(v) { var n = parseFloat(String(v).replace(/[^0-9.eE+-]/g, '')); return isNaN(n) ? null : n; }
    function isNumericCol(cells) {
      var n = 0, total = 0;
      for (var i = 0; i < cells.length; i++) { var p = parseNum(textContent(cells[i])); if (p !== null) { n++; } total++; }
      return total > 0 && n >= total * 0.5;
    }
    function getTableData(tbl) {
      var thead = tbl.querySelector('thead');
      var tbody = tbl.querySelector('tbody') || tbl;
      var rows = Array.from(tbody.querySelectorAll('tr'));
      if (rows.length === 0) return null;
      var headerRow = thead ? thead.querySelectorAll('tr')[0] : rows[0];
      var dataRows = thead ? rows : rows.slice(1);
      if (!headerRow) return null;
      var headerCells = headerRow.querySelectorAll('td, th');
      if (headerCells.length < 2 && dataRows.length === 0) return null;
      var numCols = headerCells.length;
      var valueCol = -1;
      for (var c = 1; c < numCols; c++) {
        var cells = [];
        dataRows.forEach(function(row) { var ce = row.querySelectorAll('td, th'); if (ce[c]) cells.push(ce[c]); });
        if (cells.length && isNumericCol(cells)) { valueCol = c; break; }
      }
      if (valueCol === -1 && dataRows.length > 0) {
        var firstDataRow = dataRows[0].querySelectorAll('td, th');
        for (var c = 1; c < firstDataRow.length; c++) {
          var cells = [];
          dataRows.forEach(function(row) { var ce = row.querySelectorAll('td, th'); if (ce[c]) cells.push(ce[c]); });
          if (isNumericCol(cells)) { valueCol = c; break; }
        }
      }
      if (valueCol === -1) return null;
      var labels = [];
      var values = [];
      var dataRowsUse = dataRows.length ? dataRows : (headerCells.length >= 2 ? [headerRow] : []);
      if (dataRowsUse.length === 0) return null;
      dataRowsUse.forEach(function(row) {
        var cells = row.querySelectorAll('td, th');
        if (cells.length > valueCol) {
          var lbl = textContent(cells[0]).slice(0, 40);
          var val = parseNum(textContent(cells[valueCol]));
          if (val !== null) { labels.push(lbl || 'Item'); values.push(val); }
        }
      });
      if (labels.length === 0 || values.every(function(v){ return v === 0; })) return null;
      var valueLabel = textContent(headerCells[valueCol]) || (headerCells[0] ? textContent(headerCells[0]) : 'Value');
      return { labels: labels, values: values, valueLabel: valueLabel };
    }
    var PALETTE = [
      'rgba(30,64,175,0.9)',   'rgba(21,128,61,0.9)',   'rgba(161,98,7,0.9)',
      'rgba(185,28,28,0.9)',  'rgba(124,58,237,0.9)',  'rgba(190,24,93,0.9)',
      'rgba(14,116,144,0.9)',  'rgba(194,65,12,0.9)',   'rgba(55,65,81,0.9)'
    ];
    var GRID = { color: 'rgba(0,0,0,0.08)', lineWidth: 1 };
    var FONT = { family: "'Times New Roman', Georgia, serif", size: 12 };
    function renderChart(canvas, data, figureNum) {
      var hasNeg = data.values.some(function(v){ return v < 0; });
      var sum = data.values.reduce(function(a,b){ return a + b; }, 0);
      var isProportion = data.values.length >= 2 && data.values.length <= 10 && Math.abs(sum - 100) < 5;
      var usePie = isProportion && !hasNeg;
      var useLine = data.values.length >= 5 && !hasNeg && data.labels.some(function(l){ return /\\d{4}|year|month|week|day|q\\d|time|period/i.test(l); });
      var useHorizontal = data.labels.length > 8 || data.labels.some(function(l){ return l.length > 20; });
      var colors = data.values.map(function(_, i){ return PALETTE[i % PALETTE.length]; });
      var borderColors = colors.map(function(c){ return c.replace('0.9','1'); });
      if (usePie) {
        new ChartLib(canvas, {
          type: 'doughnut',
          data: {
            labels: data.labels,
            datasets: [{ data: data.values, backgroundColor: colors, borderColor: '#fff', borderWidth: 2 }]
          },
          options: {
            responsive: true,
            maintainAspectRatio: true,
            animation: { duration: 800 },
            plugins: {
              legend: { position: 'right', labels: { font: FONT, padding: 12 } },
              title: { display: true, text: 'Figure ' + figureNum + '. ' + data.valueLabel, font: { size: 14, weight: 'bold' } }
            },
            layout: { padding: 20 }
          }
        });
        return;
      }
      if (useLine) {
        new ChartLib(canvas, {
          type: 'line',
          data: {
            labels: data.labels,
            datasets: [{
              label: data.valueLabel,
              data: data.values,
              borderColor: PALETTE[0],
              backgroundColor: 'rgba(30,64,175,0.1)',
              borderWidth: 2,
              fill: true,
              tension: 0.3,
              pointBackgroundColor: PALETTE[0],
              pointRadius: 4,
              pointHoverRadius: 6
            }]
          },
          options: {
            responsive: true,
            maintainAspectRatio: true,
            animation: { duration: 800 },
            plugins: {
              legend: { display: false },
              title: { display: true, text: 'Figure ' + figureNum + '. ' + data.valueLabel, font: { size: 14, weight: 'bold' } }
            },
            scales: {
              y: { beginAtZero: true, grid: GRID, ticks: { font: FONT, maxTicksLimit: 10 } },
              x: { grid: GRID, ticks: { font: FONT, maxRotation: 45, minRotation: 0, maxTicksLimit: 12 } }
            },
            layout: { padding: 20 }
          }
        });
        return;
      }
      var type = useHorizontal ? 'bar' : 'bar';
      var opts = {
        indexAxis: useHorizontal ? 'y' : 'x',
        responsive: true,
        maintainAspectRatio: true,
        animation: { duration: 800 },
        plugins: {
          legend: { display: false },
          title: { display: true, text: 'Figure ' + figureNum + '. ' + data.valueLabel, font: { size: 14, weight: 'bold' } }
        },
        scales: {
          x: { grid: GRID, ticks: { font: FONT, maxRotation: 45, minRotation: 0, maxTicksLimit: useHorizontal ? 20 : 10 } },
          y: { beginAtZero: !hasNeg, grid: GRID, ticks: { font: FONT, maxTicksLimit: 10 } }
        },
        layout: { padding: 20 }
      };
      if (useHorizontal) {
        opts.scales = {
          x: { beginAtZero: !hasNeg, grid: GRID, ticks: { font: FONT, maxTicksLimit: 10 } },
          y: { grid: GRID, ticks: { font: FONT, maxTicksLimit: 15 } }
        };
      }
      new ChartLib(canvas, {
        type: type,
        data: {
          labels: data.labels,
          datasets: [{
            label: data.valueLabel,
            data: data.values,
            backgroundColor: colors,
            borderColor: borderColors,
            borderWidth: 1
          }]
        },
        options: opts
      });
    }
    var root = document.querySelector('.formal-research-report');
    if (!root) return;
    var tables = root.querySelectorAll('table');
    var figureIndex = 0;
    tables.forEach(function(tbl) {
      var data = getTableData(tbl);
      if (!data) return;
      figureIndex++;
      var wrapper = document.createElement('div');
      wrapper.className = 'figure-wrapper';
      var chartContainer = document.createElement('div');
      chartContainer.className = 'chart-container';
      var canvas = document.createElement('canvas');
      canvas.setAttribute('role', 'img');
      canvas.setAttribute('aria-label', 'Chart: Figure ' + figureIndex + ' ' + data.valueLabel);
      chartContainer.appendChild(canvas);
      var caption = document.createElement('p');
      caption.className = 'figure-caption';
      caption.textContent = 'Figure ' + figureIndex + '. ' + data.valueLabel;
      wrapper.appendChild(chartContainer);
      wrapper.appendChild(caption);
      tbl.parentNode.insertBefore(wrapper, tbl);
      renderChart(canvas, data, figureIndex);
    });
  }
  loadScript('https://cdn.jsdelivr.net/npm/chart.js@4.4.1/dist/chart.umd.min.js', runCharts);
})();
<\\/script>`;

/**
 * Injects formal research styling and Chart.js (loaded dynamically) into report HTML.
 * Every table in the report gets a high-quality chart above it (bar, line, or doughnut by data).
 */
export function enhanceFormalReportHTML(rawHtml: string, reportTitle?: string): string {
  if (!rawHtml || typeof rawHtml !== 'string') return rawHtml;

  const hasStructure = /<\s*html|<\s*head|<\s*body/i.test(rawHtml);

  if (hasStructure) {
    let out = rawHtml;

    // Inject CSS before </head> (no Chart in head; we load it in script)
    if (/<\s*\/\s*head\s*>/i.test(out)) {
      out = out.replace(/<\s*\/\s*head\s*>/i, '<style>' + FORMAL_RESEARCH_CSS + '</style></head>');
    } else {
      out = out.replace(/<\s*head\s*>/i, '<head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>' + (reportTitle || 'Research Report') + '</title><style>' + FORMAL_RESEARCH_CSS + '</style>');
    }

    // Add class to body for formal styling
    if (!/formal-research-report/i.test(out)) {
      out = out.replace(/<\s*body(\s[^>]*)?>/i, (match) => {
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
  return '<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>' + escapeHtml(title) + '</title><style>' + FORMAL_RESEARCH_CSS + '</style></head><body class="formal-research-report">' + rawHtml + CHART_SCRIPT + '</body></html>';
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}
