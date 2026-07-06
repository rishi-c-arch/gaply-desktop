// FIXTURE — intentionally contains a FAKE API key to prove the scanner
// catches leaked secrets. This lives outside src/ so the real check:secrets
// (which scans src/) never trips on it.
export const CLAUDE_KEY = "sk-ant-api03-AAAABBBBCCCCDDDDEEEEFFFFGGGGHHHHIIIIJJJJ1234";
