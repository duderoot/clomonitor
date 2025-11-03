-- delete_component_mapping deletes a component mapping
-- Input: {
--   "purl": "pkg:maven/org.example/library@1.0.0"  -- required
-- }
-- Returns: success message with deleted purl
create or replace function delete_component_mapping(p_input jsonb)
returns jsonb as $$
declare
    v_purl text;
    v_deleted_record record;
    v_exists boolean;
begin
    -- Extract and validate required fields
    v_purl := p_input->>'purl';
    if v_purl is null or trim(v_purl) = '' then
        raise exception 'purl is required to identify the mapping to delete';
    end if;

    -- Check if mapping exists and get details before deletion
    select
        purl,
        repository_url,
        mapping_source
    into v_deleted_record
    from dt_component_mapping
    where purl = v_purl;

    if v_deleted_record is null then
        raise exception 'Component mapping with purl "%" not found', v_purl;
    end if;

    -- Delete the mapping
    delete from dt_component_mapping
    where purl = v_purl;

    -- Return success message with deleted record info
    return jsonb_build_object(
        'success', true,
        'deleted_purl', v_deleted_record.purl,
        'repository_url', v_deleted_record.repository_url,
        'mapping_source', v_deleted_record.mapping_source,
        'message', 'Component mapping deleted successfully'
    );
end;
$$ language plpgsql;
