/**
 * Locally bundled fonts. Replaces the Google Fonts CDN <link> tags that were
 * in public/index.html and the @import rules in hero-new.css /
 * PremiumHero.css, so the app renders correctly offline and inside the
 * Tauri desktop shell (strict CSP, no external hosts).
 *
 * Families and weights mirror what the CDN links loaded.
 */
// Inter 300–900 (body/UI)
import '@fontsource/inter/300.css';
import '@fontsource/inter/400.css';
import '@fontsource/inter/500.css';
import '@fontsource/inter/600.css';
import '@fontsource/inter/700.css';
import '@fontsource/inter/800.css';
import '@fontsource/inter/900.css';
// Orbitron 400–900 (logo)
import '@fontsource/orbitron/400.css';
import '@fontsource/orbitron/500.css';
import '@fontsource/orbitron/600.css';
import '@fontsource/orbitron/700.css';
import '@fontsource/orbitron/800.css';
import '@fontsource/orbitron/900.css';
// Space Grotesk 300–700 (dashboard)
import '@fontsource/space-grotesk/300.css';
import '@fontsource/space-grotesk/400.css';
import '@fontsource/space-grotesk/500.css';
import '@fontsource/space-grotesk/600.css';
import '@fontsource/space-grotesk/700.css';
// Epilogue 400/700/800/900 + 400 italic
import '@fontsource/epilogue/400.css';
import '@fontsource/epilogue/400-italic.css';
import '@fontsource/epilogue/700.css';
import '@fontsource/epilogue/800.css';
import '@fontsource/epilogue/900.css';
// Manrope 400–700
import '@fontsource/manrope/400.css';
import '@fontsource/manrope/500.css';
import '@fontsource/manrope/600.css';
import '@fontsource/manrope/700.css';
// Noto Serif 400–700 (Citation Manager headlines / display)
import '@fontsource/noto-serif/400.css';
import '@fontsource/noto-serif/500.css';
import '@fontsource/noto-serif/600.css';
import '@fontsource/noto-serif/700.css';
// Public Sans 400–700 (Citation Manager labels)
import '@fontsource/public-sans/400.css';
import '@fontsource/public-sans/500.css';
import '@fontsource/public-sans/600.css';
import '@fontsource/public-sans/700.css';
// Citation Manager (Archive design, from the Stitch export):
// Libre Caslon Text — headlines (both themes) + all dark-theme text
import '@fontsource/libre-caslon-text/400.css';
import '@fontsource/libre-caslon-text/400-italic.css';
import '@fontsource/libre-caslon-text/700.css';
// Source Serif 4 — light-theme body
import '@fontsource/source-serif-4/400.css';
import '@fontsource/source-serif-4/500.css';
import '@fontsource/source-serif-4/600.css';
import '@fontsource/source-serif-4/700.css';
// Hanken Grotesk — light-theme labels / small UI
import '@fontsource/hanken-grotesk/400.css';
import '@fontsource/hanken-grotesk/500.css';
import '@fontsource/hanken-grotesk/600.css';
import '@fontsource/hanken-grotesk/700.css';
// Material Symbols Outlined (variable font: wght + FILL axes)
import 'material-symbols/outlined.css';
