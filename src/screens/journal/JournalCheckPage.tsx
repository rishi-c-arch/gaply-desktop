// Gaply — Journal Check (F9). Journal/conference legitimacy + QUALITY QUARTILE
// checker on INTERNATIONAL standards: SJR (Scimago) Q1–Q4 + Scopus / Web of
// Science indexing. No national list is used anywhere.
import React, { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  AppShell,
  Badge,
  Button,
  Card,
  GaplyGlobe,
  HeaderBar,
  NavRail,
  Panel,
} from '../../design-system';
import {
  JournalRecord,
  JOURNAL_COUNT,
  riskVerdict,
  searchLocal,
} from './journalData';
import {
  CachedJournalLookup,
  conferenceRisk,
  ConferenceRecord,
  JournalLookupClient,
  makeMockLookup,
  ProxyJournalLookup,
  searchConference,
} from './journalLookup';
import { mayUseCloud } from '../settings/settingsStore';
import './journal.css';

export interface JournalCheckPageProps {
  /** Online fallback client (proxy in prod; mock in tests). */
  lookup?: JournalLookupClient;
}

const JournalCheckPage: React.FC<JournalCheckPageProps> = ({ lookup }) => {
  const navigate = useNavigate();
  const cachedLookup = useMemo(
    () => new CachedJournalLookup(lookup ?? new ProxyJournalLookup('/proxy')),
    [lookup]
  );

  const [tab, setTab] = useState<'journal' | 'conference'>('journal');
  const [query, setQuery] = useState('');
  const [result, setResult] = useState<JournalRecord | null>(null);
  const [conf, setConf] = useState<ConferenceRecord | null>(null);
  const [suggestions, setSuggestions] = useState<JournalRecord[]>([]);
  const [confSuggest, setConfSuggest] = useState<ConferenceRecord[]>([]);
  const [status, setStatus] = useState<'idle' | 'searching' | 'notfound' | 'cloudoff' | 'onlineerror'>('idle');
  const [fromCacheOrOnline, setFromCacheOrOnline] = useState<'local' | 'online' | null>(null);

  const runJournalSearch = async () => {
    setConf(null);
    const local = searchLocal(query);
    if (local.length === 1) {
      setResult(local[0]);
      setSuggestions([]);
      setFromCacheOrOnline('local');
      setStatus('idle');
      return;
    }
    if (local.length > 1) {
      setResult(null);
      setSuggestions(local);
      setStatus('idle');
      return;
    }
    // not in the local directory → online fallback (via proxy), TTL-cached.
    // The F14 privacy toggle gates this: off means NO network call at all.
    if (!mayUseCloud('journal_check')) {
      setResult(null);
      setSuggestions([]);
      setStatus('cloudoff');
      return;
    }
    setStatus('searching');
    try {
      const online = await cachedLookup.lookup(query);
      if (online) {
        setResult(online);
        setFromCacheOrOnline('online');
        setStatus('idle');
      } else {
        // A real lookup ran and returned nothing → genuinely not found.
        setResult(null);
        setStatus('notfound');
      }
      setSuggestions([]);
    } catch (e) {
      // The lookup ERRORED (e.g. the online endpoint isn't available). Do NOT
      // claim "not found" — that would be a false negative. Say the online
      // check couldn't run.
      console.error('[journal] online lookup failed:', e instanceof Error ? e.message : e);
      setResult(null);
      setSuggestions([]);
      setStatus('onlineerror');
    }
  };

  const runConferenceSearch = () => {
    setResult(null);
    const found = searchConference(query);
    if (found.length === 1) {
      setConf(found[0]);
      setConfSuggest([]);
    } else {
      setConf(null);
      setConfSuggest(found);
    }
  };

  const onSearch = () => (tab === 'journal' ? runJournalSearch() : runConferenceSearch());

  return (
    <div className="gds-root" style={{ height: '100vh', overflow: 'auto' }} data-testid="journal-check">
      <AppShell
        rail={
          <NavRail
            items={[
              { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
              { id: 'journal', label: 'Journal Check', icon: '◈' },
            ]}
            activeId="journal"
            brand={<GaplyGlobe scale="mark" />}
          />
        }
        header={<HeaderBar title="Journal Check"><Badge status="certain">SJR · Scopus · WoS</Badge></HeaderBar>}
      >
        <Panel title="Journal & conference legitimacy — international standards">
          <div className="gds-jc">
            <div className="gds-jc__tabs" role="tablist">
              <button role="tab" aria-selected={tab === 'journal'} className="gds-jc__tab" data-testid="tab-journal" onClick={() => setTab('journal')}>Journal</button>
              <button role="tab" aria-selected={tab === 'conference'} className="gds-jc__tab" data-testid="tab-conference" onClick={() => setTab('conference')}>Conference</button>
            </div>

            <div className="gds-jc__search">
              <input
                className="gds-jc__input"
                placeholder={tab === 'journal' ? 'Enter journal name, ISSN, or URL' : 'Enter conference name or acronym (e.g. NeurIPS)'}
                value={query}
                data-testid="search-input"
                onChange={(e) => setQuery(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && onSearch()}
              />
              <Button onClick={onSearch} data-testid="search-btn">Check</Button>
            </div>
            <p className="gds-jc__disclaimer">
              Based on international indexing & quartiles (SJR/Scimago, Scopus, Web of Science) —
              not any national list. Local directory: {JOURNAL_COUNT} journals.
            </p>

            {/* journal result */}
            {tab === 'journal' && result && <JournalResultCard journal={result} origin={fromCacheOrOnline} onFit={() => navigate('/app/report')} />}
            {tab === 'journal' && suggestions.length > 0 && (
              <div className="gds-jc-suggest" data-testid="suggestions">
                {suggestions.map((j) => (
                  <button key={j.name} onClick={() => { setResult(j); setSuggestions([]); setFromCacheOrOnline('local'); }}>
                    {j.name} · {j.quartile ?? '—'}
                  </button>
                ))}
              </div>
            )}
            {tab === 'journal' && status === 'searching' && <p className="gds-jc__disclaimer" data-testid="searching">Checking online directory…</p>}
            {tab === 'journal' && status === 'notfound' && <p className="gds-jc__disclaimer" data-testid="notfound">Not found locally or online.</p>}
            {tab === 'journal' && status === 'cloudoff' && (
              <p className="gds-jc__disclaimer" data-testid="cloud-off">
                Not in the offline directory — online lookup is turned off in Settings → Sync &amp; Privacy.
              </p>
            )}
            {tab === 'journal' && status === 'onlineerror' && (
              <p className="gds-jc__disclaimer" data-testid="online-error">
                Not in the offline directory. The online lookup couldn’t run right now, so we can’t
                confirm this journal either way — please try again later.
              </p>
            )}

            {/* conference result */}
            {tab === 'conference' && conf && <ConferenceResultCard conf={conf} />}
            {tab === 'conference' && confSuggest.length > 0 && (
              <div className="gds-jc-suggest" data-testid="conf-suggestions">
                {confSuggest.map((c) => (
                  <button key={c.acronym} onClick={() => { setConf(c); setConfSuggest([]); }}>{c.name} ({c.acronym})</button>
                ))}
              </div>
            )}
          </div>
        </Panel>
      </AppShell>
    </div>
  );
};

/* ---------------------------- journal result ---------------------------- */

const JournalResultCard: React.FC<{ journal: JournalRecord; origin: 'local' | 'online' | null; onFit: () => void }> = ({ journal, origin, onFit }) => {
  const risk = riskVerdict(journal);
  return (
    <div className="gds-jc-card" data-testid="result-card">
      <div className="gds-jc-card__head">
        <div>
          <h2 className="gds-jc-card__name">{journal.name}</h2>
          <div className="gds-jc-card__pub">{journal.publisher}{origin === 'online' && ' · online lookup'}</div>
        </div>
        {journal.quartile && (
          <div className="gds-quartile" data-q={journal.quartile} data-testid="quartile-badge" title={`Scimago ${journal.quartile}`}>
            {journal.quartile}
          </div>
        )}
      </div>

      {/* predatory-risk traffic light */}
      <div className="gds-risk" data-level={risk.level} data-testid="risk-verdict">
        <span className="gds-risk__dot" />
        <span>
          {risk.level === 'green' ? 'Legitimate — well indexed' : risk.level === 'amber' ? 'Caution — verify fit' : 'High predatory risk'}
          {risk.reasons.length > 0 && `: ${risk.reasons.join('; ')}`}
        </span>
      </div>

      <div className="gds-jc-grid">
        <div className="gds-jc-field"><label>SJR (Scimago)</label><span data-testid="sjr">{journal.sjr != null ? journal.sjr.toFixed(3) : `${journal.quartile ?? '—'} quartile`}</span></div>
        <div className="gds-jc-field"><label>ISSN</label><span className="gds-mono">{journal.issn ?? '—'}</span></div>
        <div className="gds-jc-field">
          <label>Indexing</label>
          <span className="gds-jc-index">
            <Badge status={journal.indexing.scopus ? 'certain' : 'flagged'}>Scopus {journal.indexing.scopus ? '✓' : '✗'}</Badge>
            <Badge status={journal.indexing.wos ? 'certain' : 'neutral'}>WoS {journal.indexing.wos ? '✓' : '—'}</Badge>
          </span>
        </div>
        <div className="gds-jc-field"><label>Category</label><span>{journal.category || '—'}</span></div>
      </div>

      <div className="gds-jc-field">
        <label>Author guidelines</label>
        {journal.guidelinesUrl ? <a href={journal.guidelinesUrl} target="_blank" rel="noreferrer" style={{ color: 'var(--g-accent)' }}>{journal.guidelinesUrl}</a> : <span>—</span>}
      </div>

      {/* PublishReady tie-in */}
      <Button variant="secondary" onClick={onFit} data-testid="journal-fit">
        Which quartile journals is my paper competitive for? →
      </Button>
    </div>
  );
};

/* --------------------------- conference result -------------------------- */

const ConferenceResultCard: React.FC<{ conf: ConferenceRecord }> = ({ conf }) => {
  const level = conferenceRisk(conf);
  return (
    <div className="gds-jc-card" data-testid="conf-card">
      <div className="gds-jc-card__head">
        <div>
          <h2 className="gds-jc-card__name">{conf.name}</h2>
          <div className="gds-jc-card__pub">{conf.acronym}</div>
        </div>
        <Badge status={level === 'green' ? 'certain' : level === 'amber' ? 'assessed' : 'flagged'} data-testid="core-rank">
          CORE {conf.coreRank}
        </Badge>
      </div>
      <div className="gds-risk" data-level={level} data-testid="conf-risk">
        <span className="gds-risk__dot" />
        <span>
          {level === 'green' ? 'Reputable venue (CORE A*/A)' : level === 'amber' ? 'Mid-tier (CORE B) — verify fit' : 'Unranked / low — verify legitimacy'}
        </span>
      </div>
      <p className="gds-jc__disclaimer">Legitimacy from CORE-ranking-style signals — an international conference standard.</p>
    </div>
  );
};

export default JournalCheckPage;
