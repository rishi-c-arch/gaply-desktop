/**
 * Edge middleware: redirect gaply.in (apex) to www.gaply.in
 * Fixes blank page when users visit gaply.in/final-orchestrator
 */
export default function middleware(request) {
  const url = new URL(request.url);
  if (url.hostname === 'gaply.in') {
    const dest = `https://www.gaply.in${url.pathname}${url.search}${url.hash}`;
    return new Response(null, { status: 308, headers: { Location: dest } });
  }
  return; // pass through for www.gaply.in
}
