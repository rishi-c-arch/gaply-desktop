# Trusted Execution Environment — AWS Nitro Enclaves

A hardening path for **sensitive summary processing** on the server side. The
proxy is already hidden from the public internet (bind_guard + Tailscale), but
the **host still sees the Claude API key and the summaries in cleartext**. A
Nitro Enclave removes the host from the trust boundary.

> **Status — designed & unit-tested now; deploy/verify needs real AWS.**
> The attestation *verification policy* (`app/enclave.py`) and the attest→verify→
> forward handshake (`app/enclave_client.py`) are implemented and unit-tested
> with mocked crypto (`tests/test_enclave.py`). Building the enclave image,
> obtaining a genuine attestation from the Nitro Security Module (NSM), the real
> CBOR/COSE_Sign1 ES384 signature, the AWS Nitro root-CA chain, KMS
> attestation-gated key release, and VSOCK transport **all require an AWS account
> with a Nitro-Enclave-capable EC2 instance** (e.g. `m5.xlarge`+, enclave option
> enabled). None of that can be exercised in this repo or in CI.

## Why Nitro (vs the alternatives)

- **AWS Nitro Enclaves (primary recommendation).** Multi-vendor, virtualization-
  based isolation carved from the parent EC2 instance. The parent has **zero
  visibility into enclave memory**; there is no persistent storage, no
  interactive access, and **no network** — the only I/O is a **VSOCK** channel.
  Cryptographic **attestation** (NSM) proves *which code* is running inside.
- **Apple Secure Enclave — future on-device pass (documented, not built here).**
  For *on-device* sensitive operations in the macOS/Tauri app (e.g. sealing a
  local key, biometric-gated signing) the Apple Secure Enclave is the right tool:
  keys are generated in and never leave the SEP; use the Secure Enclave via
  `SecKeyCreateRandomKey` with `kSecAttrTokenIDSecureEnclave` (P-256 only) and
  gate use behind LocalAuthentication. This is a **separate, future macOS-specific
  hardening pass** — it protects the *client*, not this server-side proxy.
- **Intel SGX — not the primary recommendation.** Enclave-in-process model, but:
  (1) **EPC memory limits** — usable enclave memory is small (tens/low-hundreds of
  MB before costly paging), awkward for general workloads; (2) a **history of
  side-channel/microarchitectural attacks** (Foreshadow/L1TF, SGAxe, Plundervolt,
  ÆPIC Leak) with a heavy microcode-patch treadmill; (3) SGX was **deprecated on
  mainstream client CPUs**. Nitro's coarser VM-level boundary sidesteps the EPC
  size problem and the in-core side-channel class, which is why it's preferred here.

## Architecture

```
  tailnet (Tailscale, private)
        │  structured summary (JSON)
        ▼
  ┌───────────────┐   VSOCK    ┌──────────────────────────────┐
  │  proxy        │──────────► │  Nitro Enclave               │
  │ (parent EC2)  │ 1 attest   │  • holds Claude API key      │
  │  bind_guard   │◄────────── │    (released by KMS only if   │
  │  App Check    │  attest.doc│     attestation PCR0 matches) │
  │  rate limit   │  (verify)  │  • calls Claude (via parent   │
  │  validation   │──────────► │    vsock-proxy egress)       │
  │               │ 2 summary  │  • returns result over VSOCK  │
  └───────────────┘◄────────── └──────────────────────────────┘
   host CANNOT read enclave memory; only VSOCK bytes cross the boundary
```

The enclave has **no network of its own**. Outbound HTTPS to `api.anthropic.com`
goes through a **`vsock-proxy`** running on the parent (the parent forwards
encrypted TLS bytes but never holds the key or sees plaintext — TLS terminates
inside the enclave).

## Build process (on a Nitro-capable instance)

```bash
# 1. Install tooling and enable the enclave allocator.
sudo amazon-linux-extras install aws-nitro-enclaves-cli -y
sudo yum install aws-nitro-enclaves-cli-devel -y
sudo systemctl enable --now nitro-enclaves-allocator.service

# 2. Build the enclave app container, then convert it to an Enclave Image File.
docker build -f deploy/enclave/Dockerfile.enclave -t gaply-enclave .
nitro-cli build-enclave --docker-uri gaply-enclave:latest \
  --output-file gaply-enclave.eif
# ^ prints the measurements. PCR0 = hash of the EIF = the CODE IDENTITY.
#   Example output:
#   {
#     "Measurements": {
#       "PCR0": "a1b2c3…(96 hex chars)…",   <-- pin this
#       "PCR1": "…", "PCR2": "…"
#     }
#   }

# 3. Pin PCR0 for the proxy's verifier.
export GAPLY_ENCLAVE_PCR0=a1b2c3…      # from the build output above
export GAPLY_ENCLAVE_ENABLED=yes

# 4. Run the enclave and note its VSOCK context id (CID).
nitro-cli run-enclave --eif-path gaply-enclave.eif \
  --cpu-count 2 --memory 512 --enclave-cid 16
nitro-cli describe-enclaves      # -> EnclaveCID, e.g. 16
export GAPLY_ENCLAVE_CID=16
```

**Reproducible builds matter:** PCR0 must be deterministic so the value you pin
equals the value the deployed enclave reports. Pin the base image by digest and
avoid embedding timestamps in the image.

## Key release — KMS gated by attestation (no key on the host, ever)

The API key is stored encrypted (KMS / Secrets Manager). The enclave calls
`kms:Decrypt` **with its attestation document attached**; the KMS key policy only
allows decryption when the attestation matches the expected image:

```jsonc
// KMS key policy condition — release the key ONLY to this exact enclave image
"Condition": {
  "StringEqualsIgnoreCase": {
    "kms:RecipientAttestation:PCR0": "a1b2c3…(the pinned PCR0)…"
  }
}
```

So even a fully root-compromised parent host cannot obtain the key: KMS refuses
to release it to anything whose PCR0 differs from the audited enclave image.

## Attestation verification flow (what `app/enclave.py` does)

1. Proxy generates a random `nonce`, sends `{op:"attest", nonce}` over VSOCK.
2. Enclave asks the **NSM** for an attestation document (CBOR/COSE_Sign1, ES384),
   embedding its PCRs, a cert chain to the AWS Nitro root, the echoed `nonce`, and
   a timestamp; returns it.
3. Proxy verifies, in order: **cert chain → AWS Nitro root** (root of trust),
   **COSE signature** (authenticity/anti-tamper), **PCR0 == pinned** (code
   identity), **nonce match** (freshness/anti-replay), **timestamp within skew**
   (freshness). Only on success does it send `{op:"complete", summary}`.
4. Enclave calls Claude with the in-enclave key and returns `{result}`.

In `app/enclave.py` the COSE signature is mocked as HMAC and the AWS root as a
pinned fingerprint, so the **policy** is unit-tested offline. Production swaps the
`SignatureCheck` seam for real COSE_Sign1/ES384 + x509 chain validation
(`aws-nitro-enclaves-nsm-api`, `cose`, `cryptography`).

## What's real now vs what needs AWS

| Piece | Status |
|---|---|
| Attestation policy: fields, root pinning, PCR0 code-identity, nonce, freshness | **Implemented + unit-tested** (`app/enclave.py`, `tests/test_enclave.py`) |
| ACCEPT valid doc / REJECT tampered PCR0, signature, payload, root, nonce, staleness | **Unit-tested** (mock crypto) |
| attest → verify → forward handshake; blocks forward on failed attestation | **Implemented + unit-tested** (`app/enclave_client.py`) |
| Real CBOR/COSE_Sign1 ES384 decode + AWS Nitro root-CA chain | **Documented seam** — needs the real NSM doc + AWS root |
| EIF build, PCR0 measurement, `nitro-cli` run, VSOCK transport | **Documented** — needs a Nitro-capable EC2 instance |
| KMS attestation-gated key release | **Documented** — needs a real KMS key + policy |
| End-to-end (enclave holds key, calls Claude, host blind) | **Requires real AWS to verify** |
