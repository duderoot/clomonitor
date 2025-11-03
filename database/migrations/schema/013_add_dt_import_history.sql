-- Create table for tracking DT import runs
CREATE TABLE IF NOT EXISTS dt_import_history (
    id SERIAL PRIMARY KEY,
    foundation_id TEXT NOT NULL REFERENCES foundation(foundation_id) ON DELETE CASCADE,
    import_timestamp TIMESTAMPTZ DEFAULT NOW(),
    components_total INT NOT NULL,
    components_mapped INT NOT NULL,
    components_unmapped INT NOT NULL,
    projects_registered INT NOT NULL,
    duration_seconds DECIMAL(10, 2),
    success_rate DECIMAL(5, 2) NOT NULL CHECK (success_rate >= 0 AND success_rate <= 100),
    import_metadata JSONB
);

-- Create indexes for common queries
CREATE INDEX idx_dt_import_history_foundation ON dt_import_history(foundation_id);
CREATE INDEX idx_dt_import_history_timestamp ON dt_import_history(import_timestamp DESC);

-- Add comment
COMMENT ON TABLE dt_import_history IS 'Tracks history of DT component import runs';
