-- update_component_mapping updates an existing component mapping
-- Input: {
--   "purl": "pkg:maven/org.example/library@1.0.0",  -- required, identifies the mapping
--   "repository_url": "https://github.com/example/new-url",  -- optional
--   "notes": "Updated URL after project moved",  -- optional
--   "mapping_source": "override",  -- optional
--   "verified": true,  -- optional
--   "confidence_score": 90  -- optional
-- }
-- Returns: updated mapping record as JSONB
create or replace function update_component_mapping(p_input jsonb)
returns jsonb as $$
declare
    v_purl text;
    v_repository_url text;
    v_mapping_source text;
    v_notes text;
    v_verified boolean;
    v_confidence_score smallint;
    v_result jsonb;
    v_exists boolean;
begin
    -- Extract and validate required fields
    v_purl := p_input->>'purl';
    if v_purl is null or trim(v_purl) = '' then
        raise exception 'purl is required to identify the mapping to update';
    end if;

    -- Check if mapping exists
    select exists(select 1 from dt_component_mapping where purl = v_purl) into v_exists;
    if not v_exists then
        raise exception 'Component mapping with purl "%" not found', v_purl;
    end if;

    -- Extract optional update fields
    v_repository_url := p_input->>'repository_url';
    v_mapping_source := p_input->>'mapping_source';
    v_notes := p_input->>'notes';

    -- Handle boolean field
    if p_input ? 'verified' then
        v_verified := (p_input->>'verified')::boolean;
    end if;

    -- Handle confidence_score with validation
    if p_input->>'confidence_score' is not null then
        v_confidence_score := (p_input->>'confidence_score')::smallint;
        if v_confidence_score < 0 or v_confidence_score > 100 then
            raise exception 'confidence_score must be between 0 and 100';
        end if;
    end if;

    -- Validate repository URL if provided
    if v_repository_url is not null then
        if trim(v_repository_url) = '' then
            raise exception 'repository_url cannot be empty';
        end if;
        if v_repository_url !~ '^https?://([^/]+\.)?(github\.com|gitlab\.com|bitbucket\.org|gitee\.com)/' then
            raise exception 'repository_url must be a valid Git hosting URL (GitHub, GitLab, Bitbucket, or Gitee)';
        end if;
    end if;

    -- Update the mapping, only updating provided fields
    update dt_component_mapping
    set
        repository_url = coalesce(v_repository_url, repository_url),
        mapping_source = coalesce(v_mapping_source, mapping_source),
        notes = coalesce(v_notes, notes),
        verified = coalesce(v_verified, verified),
        confidence_score = coalesce(v_confidence_score, confidence_score),
        updated_at = now()
    where purl = v_purl
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
end;
$$ language plpgsql;
