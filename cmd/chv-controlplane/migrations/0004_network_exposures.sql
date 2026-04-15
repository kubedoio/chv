ALTER TABLE network_desired_state
    DROP COLUMN IF EXISTS service_name,
    DROP COLUMN IF EXISTS protocol,
    DROP COLUMN IF EXISTS listen_address,
    DROP COLUMN IF EXISTS listen_port,
    DROP COLUMN IF EXISTS target_address,
    DROP COLUMN IF EXISTS target_port,
    DROP COLUMN IF EXISTS exposure_policy;

CREATE TABLE IF NOT EXISTS network_exposures (
    network_exposure_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    network_id text NOT NULL REFERENCES networks (network_id) ON DELETE CASCADE,
    service_name text NOT NULL,
    protocol text NOT NULL,
    listen_address inet,
    listen_port integer,
    target_address inet,
    target_port integer,
    exposure_policy text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (network_id, service_name)
);

CREATE INDEX IF NOT EXISTS network_exposures_network_id_idx ON network_exposures (network_id);
