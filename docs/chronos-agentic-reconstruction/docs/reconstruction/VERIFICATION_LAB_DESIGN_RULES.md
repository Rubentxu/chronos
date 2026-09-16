# Verification Lab Design Rules

1. Use the cheapest environment that proves the contract.
2. Require explicit capability negotiation.
3. Keep sandbox backend dependencies out of domain/application semantics.
4. Keep observer and subject state physically separate.
5. Treat cleanup as part of acceptance.
6. Keep Coverage and runtime verification as separate quality surfaces.
7. Do not create sandbox infrastructure without a roadmap consumer.
8. Do not expose portable execution publicly until dogfooding proves the seam.
