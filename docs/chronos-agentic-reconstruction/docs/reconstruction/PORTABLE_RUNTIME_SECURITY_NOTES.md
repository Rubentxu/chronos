# Portable Runtime — Security Notes

Status: FUTURE / NON-NORMATIVE

A future portable or remote execution runtime must not inherit secrets, network access or host privilege implicitly.

Minimum future concerns:

- explicit secret mounts;
- deny-by-default network policy for sandbox profiles;
- authenticated remote runtime transport;
- least-privilege Linux capabilities;
- immutable/read-only inputs where practical;
- disposable runtime filesystem;
- artifact/evidence redaction policy;
- no blanket host filesystem mounts;
- runtime image/digest attestation or provenance;
- cancellation and orphan cleanup.

A dedicated security ADR is required before PORTABLE-P2 remote execution implementation.
