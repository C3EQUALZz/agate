-- Transparency-log persistence for the audit context.

CREATE TABLE audit_log (
    id         UUID     PRIMARY KEY,
    created_at BIGINT   NOT NULL, -- Unix milliseconds
    updated_at BIGINT   NOT NULL,
    hash_algo  SMALLINT NOT NULL  -- HashAlgo::code (epoch algorithm)
);

CREATE TABLE audit_leaf (
    log_id     UUID   NOT NULL REFERENCES audit_log (id),
    leaf_index BIGINT NOT NULL,
    leaf_hash  BYTEA  NOT NULL,
    PRIMARY KEY (log_id, leaf_index)
);
