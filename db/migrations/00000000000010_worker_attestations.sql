-- Worker attestation tracking for distributed integrity verification
CREATE TABLE worker_attestations (
    worker_id           TEXT PRIMARY KEY,
    last_ledger_head    TEXT NOT NULL,
    build_checksum      TEXT NOT NULL,
    config_version      TEXT NOT NULL,
    attested_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    quarantined         BOOLEAN NOT NULL DEFAULT false,
    quarantine_reason   TEXT,
    quarantined_at      TIMESTAMPTZ
);

CREATE INDEX idx_worker_attestations_quarantined ON worker_attestations(quarantined) WHERE quarantined = true;
CREATE INDEX idx_worker_attestations_attested_at ON worker_attestations(attested_at DESC);
