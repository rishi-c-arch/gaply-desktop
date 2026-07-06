// FIXTURE — a clean config with no secrets. Keys are never in source; they
// live in the OS keychain and are attached by the Rust core.
export const config = {
  apiKeyRef: "stored-in-os-keychain",
  model: "claude-fable-5",
  endpoint: "/api/proxy",
};
