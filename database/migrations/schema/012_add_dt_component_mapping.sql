-- Add tables to support Dependency-Track component mapping
-- This addresses the issue where components without repository URLs are lost

-- Table 1: Persistent mapping from package URLs (purls) to repository URLs
CREATE TABLE dt_component_mapping (
    purl VARCHAR(500) PRIMARY KEY,
    repository_url TEXT NOT NULL,
    mapping_source VARCHAR(50) NOT NULL,
    verified BOOLEAN DEFAULT FALSE,
    confidence_score SMALLINT CHECK (confidence_score >= 0 AND confidence_score <= 100),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    created_by VARCHAR(100),
    notes TEXT
);

-- Indexes for dt_component_mapping
CREATE INDEX idx_dt_component_mapping_verified ON dt_component_mapping(verified);
CREATE INDEX idx_dt_component_mapping_source ON dt_component_mapping(mapping_source);
CREATE INDEX idx_dt_component_mapping_updated ON dt_component_mapping(updated_at DESC);

-- Comments for dt_component_mapping
COMMENT ON TABLE dt_component_mapping IS 'Maps package URLs (purls) to repository URLs for DT components';
COMMENT ON COLUMN dt_component_mapping.purl IS 'Package URL in standard format (pkg:type/namespace/name@version)';
COMMENT ON COLUMN dt_component_mapping.mapping_source IS 'How this mapping was created: auto, manual, bulk_import, registry_api';
COMMENT ON COLUMN dt_component_mapping.confidence_score IS 'Confidence in mapping accuracy (0-100)';
COMMENT ON COLUMN dt_component_mapping.verified IS 'Has this mapping been verified by a user or automated check?';

-- Table 2: Track components that could not be mapped to repositories
CREATE TABLE dt_unmapped_components (
    id SERIAL PRIMARY KEY,
    foundation_id TEXT NOT NULL REFERENCES foundation(foundation_id) ON DELETE CASCADE,
    component_uuid TEXT,
    component_name TEXT NOT NULL,
    component_version TEXT,
    component_group TEXT,
    purl TEXT,
    component_type TEXT,
    classifier TEXT,
    external_references JSONB,
    first_seen TIMESTAMPTZ DEFAULT NOW(),
    last_seen TIMESTAMPTZ DEFAULT NOW(),
    mapping_attempts INT DEFAULT 1,
    mapping_notes TEXT,
    -- Support for ignoring internal/private components
    ignored BOOLEAN DEFAULT FALSE,
    ignore_reason VARCHAR(100),
    ignored_at TIMESTAMPTZ,
    ignored_by VARCHAR(100)
);

-- Unique constraints for dt_unmapped_components
-- Primary: foundation + purl combination
CREATE UNIQUE INDEX idx_dt_unmapped_foundation_purl
ON dt_unmapped_components(foundation_id, purl)
WHERE purl IS NOT NULL AND NOT ignored;

-- Fallback: foundation + name + version for components without purl
CREATE UNIQUE INDEX idx_dt_unmapped_foundation_name_version
ON dt_unmapped_components(foundation_id, component_name, component_version)
WHERE purl IS NULL AND NOT ignored;

-- Other indexes for dt_unmapped_components
CREATE INDEX idx_dt_unmapped_foundation ON dt_unmapped_components(foundation_id);
CREATE INDEX idx_dt_unmapped_purl ON dt_unmapped_components(purl) WHERE purl IS NOT NULL;
CREATE INDEX idx_dt_unmapped_last_seen ON dt_unmapped_components(last_seen DESC);
CREATE INDEX idx_dt_unmapped_attempts ON dt_unmapped_components(mapping_attempts);
CREATE INDEX idx_dt_unmapped_ignored ON dt_unmapped_components(ignored) WHERE NOT ignored;

-- Comments for dt_unmapped_components
COMMENT ON TABLE dt_unmapped_components IS 'Components from DT that could not be mapped to repositories';
COMMENT ON COLUMN dt_unmapped_components.mapping_attempts IS 'Number of times mapping was attempted';
COMMENT ON COLUMN dt_unmapped_components.external_references IS 'Raw external references from DT for analysis';
COMMENT ON COLUMN dt_unmapped_components.ignored IS 'If true, this component is intentionally not mapped (e.g., internal library)';
COMMENT ON COLUMN dt_unmapped_components.ignore_reason IS 'Why this component is ignored: internal, private, vendored, etc.';
