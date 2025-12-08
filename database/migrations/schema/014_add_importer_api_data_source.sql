-- Add support for importer_api data source type
--
-- This migration adds documentation for the new 'importer_api' data source type
-- that fetches projects from the clomonitor-importer service.
--
-- No schema changes are needed since foundation.data_source_type is VARCHAR(50)
-- and foundation.data_source_config is JSONB, which already support any data source type.

-- Update the data_source_type column comment to include importer_api
COMMENT ON COLUMN foundation.data_source_type IS
'Type of data source for fetching projects:
- yaml_url: Projects defined in a YAML file at a URL
- dependency_track: Projects fetched from Dependency-Track instance
- importer_api: Projects fetched from clomonitor-importer service';

-- Update the data_source_config column comment to document importer_api config
COMMENT ON COLUMN foundation.data_source_config IS
'JSONB configuration for the data source. Structure varies by data_source_type:

yaml_url:
{
  "data_url": "https://example.com/projects.yaml"
}

dependency_track:
{
  "dt_url": "https://dt.example.com",
  "dt_api_key": "secret-api-key",
  "project_name_mapping": {"dt_name": "display_name"},
  "default_maturity": "sandbox",
  "default_category": "...",
  "default_check_sets": ["code", "community", "license", "best_practices", "security"]
}

importer_api:
{
  "importer_url": "http://clomonitor-importer:8080",
  "default_maturity": "sandbox",
  "maturity_mapping": {"key": "value"},
  "default_category": "...",
  "default_check_sets": ["code", "community", "license", "best_practices", "security"],
  "page_size": 1000
}
';

-- Add a comment on the foundation table documenting all data source types
COMMENT ON TABLE foundation IS
'Stores foundation metadata and data source configuration.

Supported data source types:
1. yaml_url: Traditional YAML file at a URL
2. dependency_track: Fetches components from Dependency-Track
3. importer_api: Fetches projects from clomonitor-importer service

The data_source_config JSONB column structure varies by data_source_type.
See COMMENT on data_source_config column for detailed schema documentation.';
