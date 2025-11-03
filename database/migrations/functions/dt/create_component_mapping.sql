-- create_component_mapping creates a new component mapping
-- Input: {
--   "purl": "pkg:maven/org.example/library@1.0.0",
--   "repository_url": "https://github.com/example/library",
--   "mapping_source": "manual",  -- optional, defaults to 'manual'
--   "created_by": "user@example.com",  -- optional
--   "notes": "Manually mapped based on documentation",  -- optional
--   "verified": true,  -- optional, defaults to false
--   "confidence_score": 95  -- optional, 0-100
-- }
-- Returns: created mapping record as JSONB
create or replace function create_component_mapping(p_input jsonb)
returns jsonb as $$
declare
    v_purl text;
    v_repository_url text;
    v_mapping_source text;
    v_created_by text;
    v_notes text;
    v_verified boolean;
    v_confidence_score smallint;
    v_result jsonb;
begin
    -- Extract and validate required fields
    v_purl := p_input->>'purl';
    if v_purl is null or trim(v_purl) = '' then
        raise exception 'purl is required and cannot be empty';
    end if;

    v_repository_url := p_input->>'repository_url';
    if v_repository_url is null or trim(v_repository_url) = '' then
        raise exception 'repository_url is required and cannot be empty';
    end if;

    -- Validate repository URL format (GitHub, GitLab, or other common Git hosting)
    if v_repository_url !~ '^https?://([^/]+\.)?(github\.com|gitlab\.com|bitbucket\.org|gitee\.com)/' then
        raise exception 'repository_url must be a valid Git hosting URL (GitHub, GitLab, Bitbucket, or Gitee)';
    end if;

    -- Extract optional fields
    v_mapping_source := coalesce(p_input->>'mapping_source', 'manual');
    v_created_by := p_input->>'created_by';
    v_notes := p_input->>'notes';
    v_verified := coalesce((p_input->>'verified')::boolean, false);

    -- Validate and extract confidence_score
    if p_input->>'confidence_score' is not null then
        v_confidence_score := (p_input->>'confidence_score')::smallint;
        if v_confidence_score < 0 or v_confidence_score > 100 then
            raise exception 'confidence_score must be between 0 and 100';
        end if;
    end if;

    -- Insert the new mapping
    insert into dt_component_mapping (
        purl,
        repository_url,
        mapping_source,
        created_by,
        notes,
        verified,
        confidence_score,
        created_at,
        updated_at
    ) values (
        v_purl,
        v_repository_url,
        v_mapping_source,
        v_created_by,
        v_notes,
        v_verified,
        v_confidence_score,
        now(),
        now()
    )
    returning jsonb_build_object(
        'purl', purl,
        'repository_url', repository_url,
        'mapping_source', mapping_source,
        'created_by', created_by,
        'notes', notes,
        'verified', verified,
        'confidence_score', confidence_score,
        'created_at', to_char(created_at at time zone 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
        'updated_at', to_char(updated_at at time zone 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"')
    ) into v_result;

    return v_result;
exception
    when unique_violation then
        raise exception 'A mapping for purl "%" already exists. Use update_component_mapping to modify it.', v_purl;
    when foreign_key_violation then
        raise exception 'Invalid reference in mapping data';
end;
$$ language plpgsql;
