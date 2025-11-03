-- get_unmapped_stats returns comprehensive statistics about unmapped DT components
create or replace function get_unmapped_stats(p_input jsonb)
returns jsonb as $$
declare
    v_foundation_id text := p_input->>'foundation_id';
    v_total_unmapped int;
    v_total_mapped int;
    v_mapping_rate_percent numeric(5,2);
    v_by_foundation jsonb;
    v_by_package_type jsonb;
    v_recent_imports jsonb;
begin
    -- 1. Get total unmapped count (only non-ignored)
    select count(*)::int into v_total_unmapped
    from dt_unmapped_components
    where not ignored
        and (v_foundation_id is null or foundation_id = v_foundation_id);

    -- 2. Get count by foundation (only non-ignored)
    select coalesce(jsonb_object_agg(foundation_id, component_count), '{}'::jsonb) into v_by_foundation
    from (
        select foundation_id, count(*)::int as component_count
        from dt_unmapped_components
        where not ignored
            and (v_foundation_id is null or foundation_id = v_foundation_id)
        group by foundation_id
    ) as counts;

    -- 3. Get total mapped from latest import runs (one per foundation)
    select coalesce(sum(components_mapped), 0)::int into v_total_mapped
    from (
        select distinct on (foundation_id) components_mapped
        from dt_import_history
        where (v_foundation_id is null or foundation_id = v_foundation_id)
        order by foundation_id, import_timestamp desc
    ) as latest_imports;

    -- 4. Calculate mapping rate percentage
    if (v_total_mapped + v_total_unmapped) > 0 then
        v_mapping_rate_percent := (v_total_mapped::numeric / (v_total_mapped + v_total_unmapped)) * 100;
    else
        v_mapping_rate_percent := 0.0;
    end if;

    -- 5. Get breakdown by package type (extracted from purl)
    select coalesce(jsonb_object_agg(package_type, count), '{}'::jsonb) into v_by_package_type
    from (
        select
            coalesce(
                substring(purl from 'pkg:([^/]+)'),
                'unknown'
            ) as package_type,
            count(*)::int as count
        from dt_unmapped_components
        where not ignored
            and (v_foundation_id is null or foundation_id = v_foundation_id)
        group by package_type
        order by count desc
    ) as pkg_types;

    -- 6. Get recent import history (last 10 runs)
    select coalesce(
        jsonb_agg(
            jsonb_build_object(
                'foundation_id', foundation_id,
                'import_timestamp', to_char(import_timestamp at time zone 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
                'components_total', components_total,
                'components_mapped', components_mapped,
                'components_unmapped', components_unmapped,
                'projects_registered', projects_registered,
                'success_rate', success_rate,
                'duration_seconds', duration_seconds
            ) order by import_timestamp desc
        ),
        '[]'::jsonb
    ) into v_recent_imports
    from (
        select *
        from dt_import_history
        where (v_foundation_id is null or foundation_id = v_foundation_id)
        order by import_timestamp desc
        limit 10
    ) as recent;

    -- 7. Return complete statistics
    return jsonb_build_object(
        'total_unmapped', v_total_unmapped,
        'total_mapped', v_total_mapped,
        'mapping_rate_percent', v_mapping_rate_percent,
        'by_foundation', v_by_foundation,
        'by_package_type', v_by_package_type,
        'recent_imports', v_recent_imports
    );
end
$$ language plpgsql;
