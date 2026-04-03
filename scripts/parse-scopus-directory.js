/**
 * Parse mammoth raw text -> structured JSON for SEO guide pages.
 * Run: node scripts/parse-scopus-directory.js scripts/scopus-raw.txt > src/data/scopusDirectory.json
 */
const fs = require('fs');

const MAJOR = new Set([
  'Health Sciences',
  'Life Sciences',
  'Physical Sciences',
  'Social Sciences',
  'Arts & Humanities',
  'Engineering & Technology',
  'Environmental Sciences',
  'Business & Economics',
]);

function main() {
  const raw = fs.readFileSync(process.argv[2] || '/dev/stdin', 'utf8');
  const lines = raw.split(/\r?\n/).map((l) => l.trim());
  const journals = [];
  let category = 'General';
  let subcategory = 'General';

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (!line) continue;

    if (MAJOR.has(line)) {
      category = line;
      subcategory = line;
      continue;
    }

    // Subcategory: heading, blank, title line, blank, ISSN
    if (
      !line.includes(':') &&
      line.length > 2 &&
      line.length < 100 &&
      lines[i + 1] === '' &&
      lines[i + 2] &&
      !lines[i + 2].startsWith('ISSN:') &&
      lines[i + 3] === '' &&
      lines[i + 4] &&
      lines[i + 4].startsWith('ISSN:')
    ) {
      subcategory = line;
      continue;
    }

    if (line.startsWith('ISSN:')) {
      let t = i - 1;
      while (t >= 0 && !lines[t]) t--;
      if (t < 0) continue;
      const title = lines[t];
      if (!title || title.startsWith('ISSN:') || title.startsWith('Publisher:')) continue;
      if (title.length > 130) continue;
      if (/^Table of Contents|^Last Updated|^Verified and/i.test(title)) continue;

      const issnLine = line;
      let publisher = '';
      let website = '';
      let reviewTime = '';
      let scope = '';
      let quartile = '';
      let frequency = '';

      const mq = issnLine.match(/Quartile:\s*([^|]+)/i);
      if (mq) quartile = mq[1].trim();
      const mf = issnLine.match(/Frequency:\s*(.+)$/i);
      if (mf) frequency = mf[1].trim();

      for (let k = i + 1; k < Math.min(i + 12, lines.length); k++) {
        const L = lines[k];
        if (L.startsWith('Publisher:')) publisher = L.replace(/^Publisher:\s*/i, '').trim();
        if (L.startsWith('Website:')) website = L.replace(/^Website:\s*/i, '').trim();
        if (L.startsWith('Review Time:')) reviewTime = L.replace(/^Review Time:\s*/i, '').trim();
        if (L.startsWith('Scope:')) scope = L.replace(/^Scope:\s*/i, '').trim();
        if (L.startsWith('ISSN:')) break;
      }

      journals.push({
        title,
        category,
        subcategory,
        issnLine,
        quartile,
        frequency,
        publisher,
        website,
        reviewTime,
        scope,
      });
    }
  }

  const out = {
    generatedNote:
      'Parsed from Scopus_Journals_Directory.docx for Gaply SEO guides. Always verify APC, indexing, and timelines on the official journal site.',
    journalCount: journals.length,
    journals,
  };

  process.stdout.write(JSON.stringify(out));
}

main();
