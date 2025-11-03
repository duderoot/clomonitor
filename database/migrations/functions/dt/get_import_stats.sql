-- get_import_stats returns statistics about DT imports and unmapped components
create or replace function get_import_stats(p_input jsonb)
returns jsonb as $$
declare
    v_foundation_id text := p_input->>'foundation_id';
    v_total_unmapped int;
    v_total_mapped int;
    v_mapping_rate decimal(5, 2);
    v_by_package_type jsonb;
    v_recent_imports jsonb;
begin
    -- Get total unmapped count
    select coalesce(count(*), 0)
    into v_total_unmapped
    from dt_unmapped_components
    where not ignored
        and (v_foundation_id is null or foundation_id = v_foundation_id);

    -- Get total mapped count from dt_component_mapping
    select coalesce(count(*), 0)
    into v_total_mapped
    from dt_component_mapping;

    -- Calculate mapping rate (avoid division by zero)
    if (v_total_unmapped + v_total_mapped) > 0 then
        v_mapping_rate := round((v_total_mapped::decimal / (v_total_unmapped + v_total_mapped)) * 100, 2);
    else
        v_mapping_rate := 0.0;
    end if;

    -- Get breakdown by package type (extract from purl)
    select coalesce(
        jsonb_object_agg(
            package_type,
            count
        ),
        '{}'::jsonb
    )
    into v_by_package_type
    from (
        select
            coalesce(
                case
                    when purl like 'pkg:npm/%' then 'npm'
                    when purl like 'pkg:maven/%' then 'maven'
                    when purl like 'pkg:pypi/%' then 'pypi'
                    when purl like 'pkg:golang/%' then 'golang'
                    when purl like 'pkg:cargo/%' then 'cargo'
                    when purl like 'pkg:gem/%' then 'gem'
                    when purl like 'pkg:nuget/%' then 'nuget'
                    else 'other'
                end,
                'unknown'
            ) as package_type,
            count(*)::int as count
        from dt_unmapped_components
        where not ignored
            and (v_foundation_id is null or foundation_id = v_foundation_id)
        group by package_type
    ) as package_types;

    -- Get recent imports (last 10)
    select coalesce(
        jsonb_agg(
            jsonb_build_object(
                'foundation_id', foundation_id,
                'import_timestamp', to_char(import_timestamp at time zone 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
                'components_total', components_total,
                'components_mapped', components_mapped,
                'components_unmapped', components_unmapped,
                'projects_registered', projects_registered,
                'duration_seconds', duration_seconds,
                'success_rate', coalesce(success_rate, 0.0)
            ) order by import_timestamp desc
        ),
        '[]'::jsonb
    )
    into v_recent_imports
    from (
        select *
        from dt_import_history
        where v_foundation_id is null or foundation_id = v_foundation_id
        order by import_timestamp desc
        limit 10
    ) as recent;

    return jsonb_build_object(
        'total_unmapped', coalesce(v_total_unmapped, 0),
        'total_mapped', coalesce(v_total_mapped, 0),
        'mapping_rate_percent', coalesce(v_mapping_rate, 0.0),
        'by_package_type', v_by_package_type,
        'recent_imports', v_recent_imports
    );
end
$$ language plpgsql;
