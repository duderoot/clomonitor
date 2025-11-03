-- Add new columns for flexible data sources to support both YAML URLs and Dependency-Track

-- Add data_source_type column with default 'yaml_url' for backward compatibility
ALTER TABLE foundation
ADD COLUMN data_source_type VARCHAR(50) NOT NULL DEFAULT 'yaml_url';

-- Add data_source_config column to store configuration as JSONB
ALTER TABLE foundation
ADD COLUMN data_source_config JSONB;

-- Migrate existing foundations to new structure
-- Copy data_url values into data_source_config JSON
UPDATE foundation
SET
    data_source_type = 'yaml_url',
    data_source_config = jsonb_build_object('data_url', data_url)
WHERE data_source_config IS NULL;

-- Keep data_url column for backward compatibility
-- Do NOT drop it yet to ensure smooth transition
-- ALTER TABLE foundation DROP COLUMN data_url;

-- Add comment explaining the schema
COMMENT ON COLUMN foundation.data_source_type IS 'Type of data source: yaml_url or dependency_track';
COMMENT ON COLUMN foundation.data_source_config IS 'JSONB configuration for the data source. For yaml_url: {"data_url": "..."}, for dependency_track: {"dt_url": "...", "dt_api_key": "..."}';
