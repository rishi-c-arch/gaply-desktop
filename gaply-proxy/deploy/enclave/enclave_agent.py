"""Enclave-side agent — SKETCH / reference only (runs INSIDE the Nitro Enclave).

This is the counterpart to app/enclave_client.py. It is intentionally NOT wired
into the proxy or the test suite: it only runs inside a real Nitro Enclave, where
`/dev/nsm` (the Nitro Security Module) and `socket.AF_VSOCK` exist. It is here to
make the deploy design concrete and reviewable.

Responsibilities inside the enclave (host cannot see any of this memory):
  1. Obtain the Claude API key at runtime via KMS Decrypt WITH an attestation
     document, so KMS only releases it to this exact enclave image (PCR0). The
     key is never baked into the image and never touches the parent host.
  2. Serve VSOCK requests from the proxy:
       {op:"attest", nonce}    -> return the NSM attestation doc (nonce embedded)
       {op:"complete", summary}-> call Claude with the in-enclave key, return result
"""

from __future__ import annotations

import json
import os

# These imports only resolve inside the enclave image (see Dockerfile.enclave):
#   import socket                      # AF_VSOCK
#   from aws_nitro_enclaves_nsm_api import nsm   # /dev/nsm attestation + entropy
#   import boto3                       # KMS Decrypt with attestation
#   from anthropic import Anthropic

VSOCK_PORT = int(os.getenv("ENCLAVE_PORT", "5005"))


def get_api_key_via_kms_attestation() -> str:
    """Release the API key to THIS enclave only (KMS checks PCR0).

    Real implementation:
      att = nsm.get_attestation_document(public_key=ephemeral_pub, nonce=..., user_data=...)
      resp = kms.decrypt(CiphertextBlob=ENC_KEY, Recipient={
          'AttestationDocument': att,
          'KeyEncryptionAlgorithm': 'RSAES_OAEP_SHA_256',
      })
      # KMS returns the plaintext ENCRYPTED to the enclave's ephemeral key;
      # decrypt it in-enclave. Host never sees plaintext.
    """
    raise NotImplementedError("runs only inside a Nitro Enclave with /dev/nsm + KMS")


def handle(message: dict, api_key: str) -> dict:
    op = message.get("op")
    if op == "attest":
        # att = nsm.get_attestation_document(nonce=message["nonce"])
        # return decode_to_json(att)   # CBOR/COSE doc the proxy verifies
        raise NotImplementedError("NSM attestation requires /dev/nsm")
    if op == "complete":
        # client = Anthropic(api_key=api_key)
        # msg = client.messages.create(model=..., max_tokens=1024,
        #                              system=..., messages=[{"role":"user",
        #                              "content": json.dumps(message["summary"])}])
        # return {"result": {"model": msg.model, "text": msg.content[0].text}}
        raise NotImplementedError("Claude call happens in-enclave")
    return {"error": "unknown_op"}


def main() -> None:  # pragma: no cover - enclave-only
    import socket

    api_key = get_api_key_via_kms_attestation()
    af_vsock = socket.AF_VSOCK  # exists only on a Nitro host
    srv = socket.socket(af_vsock, socket.SOCK_STREAM)
    srv.bind((socket.VMADDR_CID_ANY, VSOCK_PORT))
    srv.listen()
    while True:
        conn, _ = srv.accept()
        with conn:
            header = conn.recv(4)
            body = conn.recv(int.from_bytes(header, "big"))
            reply = handle(json.loads(body), api_key)
            out = json.dumps(reply).encode()
            conn.sendall(len(out).to_bytes(4, "big") + out)


if __name__ == "__main__":
    main()
