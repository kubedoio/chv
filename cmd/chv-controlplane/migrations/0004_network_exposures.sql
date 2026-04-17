-- The columns service_name/protocol/etc. were never added in the SQLite schema (0001 did not include them),
-- so no DROP COLUMN statements are needed.

CREATE TABLE IF NOT EXISTS network_exposures (
    network_exposure_id text PRIMARY KEY DEFAULT (lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-4'||substr(lower(hex(randomblob(2))),2)||'-'||substr('89ab',abs(random())%4+1,1)||substr(lower(hex(randomblob(2))),2)||'-'||lower(hex(randomblob(6)))),
    network_id text NOT NULL REFERENCES networks (network_id) ON DELETE CASCADE,
    service_name text NOT NULL,
    protocol text NOT NULL,
    listen_address text,
    listen_port integer,
    target_address text,
    target_port integer,
    exposure_policy text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    UNIQUE (network_id, service_name)
);

CREATE INDEX IF NOT EXISTS network_exposures_network_id_idx ON network_exposures (network_id);
