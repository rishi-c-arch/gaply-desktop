export interface ConferenceIndia {
  id: string;
  title: string;
  url: string;
  date_text: string;
  venue: string;
  description: string;
  scholarship: string;
  year: number;
  discipline?: string;
  is_upcoming: boolean;
  source: string;
}

export interface ConferencesIndiaResponse {
  conferences: ConferenceIndia[];
  count: number;
  source: string;
  attribution: string;
  fetched_at: string;
  cache_hit: boolean;
}
