-- get_unmapped_components returns unmapped DT components with pagination and filtering
create or replace function get_unmapped_components(p_input jsonb)
returns jsonb as $$
declare
    v_foundation_id text := p_input->>'foundation_id';
    v_limit int := coalesce((p_input->>'limit')::int, 20);
    v_offset int := coalesce((p_input->>'offset')::int, 0);
    v_search text := p_input->>'search';
    v_components jsonb;
    v_total_count int;
begin
    -- Get total count
    select count(*)
    into v_total_count
    from dt_unmapped_components
    where not ignored
        and (v_foundation_id is null or foundation_id = v_foundation_id)
        and (v_search is null or
             component_name ilike '%' || v_search || '%' or
             purl ilike '%' || v_search || '%' or
             component_group ilike '%' || v_search || '%');

    -- Get components
    select coalesce(jsonb_agg(
        jsonb_build_object(
            'uuid', coalesce(component_uuid, id::text),
            'id', id,
            'foundation_id', foundation_id,
            'component_name', component_name,
            'component_version', component_version,
            'component_group', component_group,
            'purl', purl,
            'classifier', classifier,
            'mapping_attempts', coalesce(mapping_attempts, 0),
            'first_seen', to_char(first_seen at time zone 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
            'last_seen_at', to_char(last_seen at time zone 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
            'mapping_notes', mapping_notes,
            'external_references', external_references
        ) order by last_seen desc
    ), '[]'::jsonb)
    into v_components
    from (
        select *
        from dt_unmapped_components
        where not ignored
            and (v_foundation_id is null or foundation_id = v_foundation_id)
            and (v_search is null or
                 component_name ilike '%' || v_search || '%' or
                 purl ilike '%' || v_search || '%' or
                 component_group ilike '%' || v_search || '%')
        order by last_seen desc
        limit v_limit
        offset v_offset
    ) as filtered_components;

    return jsonb_build_object(
        'components', v_components,
        'total_count', coalesce(v_total_count, 0)
    );
end
$$ language plpgsql;
