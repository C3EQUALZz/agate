-- Durable record of signed checkpoints (Signed Tree Heads).
--
-- Each row is a checkpoint the operator signed and published: the committed
-- (size, root) plus the signature over it. Retained so checkpoints survive
-- restarts and can be audited for split-view/equivocation across time.

CREATE TABLE audit_checkpoint (
    log_id     UUID     NOT NULL REFERENCES audit_log (id),
    tree_size  BIGINT   NOT NULL, -- committed tree size
    root_hash  BYTEA    NOT NULL, -- Merkle root at that size
    root_algo  SMALLINT NOT NULL, -- HashAlgo::code of root_hash
    issued_at  BIGINT   NOT NULL, -- Unix milliseconds (the STH timestamp)
    sig_algo   SMALLINT NOT NULL, -- SignAlgo::code of the signature
    key_id     TEXT     NOT NULL, -- signing key identifier
    signature  BYTEA    NOT NULL, -- signature over the canonical tree head
    -- One checkpoint per (log, size): re-issuing at an unchanged size is a
    -- no-op rather than a duplicate (append-only, idempotent).
    PRIMARY KEY (log_id, tree_size)
);
