-- record_dt_import records statistics for a DT import run
create or replace function record_dt_import(p_input jsonb)
returns void as $$
declare
    v_foundation_id text;
    v_components_total int;
    v_components_mapped int;
    v_components_unmapped int;
    v_projects_registered int;
    v_duration_seconds numeric(10,2);
    v_success_rate numeric(5,2);
    v_import_metadata jsonb;
begin
    -- Extract and validate required fields
    v_foundation_id := p_input->>'foundation_id';
    if v_foundation_id is null then
        raise exception 'foundation_id is required';
    end if;

    v_components_total := (p_input->>'components_total')::int;
    if v_components_total is null then
        raise exception 'components_total is required';
    end if;

    v_components_mapped := (p_input->>'components_mapped')::int;
    if v_components_mapped is null then
        raise exception 'components_mapped is required';
    end if;

    v_components_unmapped := (p_input->>'components_unmapped')::int;
    if v_components_unmapped is null then
        raise exception 'components_unmapped is required';
    end if;

    v_projects_registered := (p_input->>'projects_registered')::int;
    if v_projects_registered is null then
        raise exception 'projects_registered is required';
    end if;

    -- Calculate success rate (percentage of mapped components)
    if v_components_total > 0 then
        v_success_rate := round((v_components_mapped::numeric / v_components_total::numeric) * 100, 2);
    else
        v_success_rate := 0;
    end if;

    -- Extract optional fields
    v_duration_seconds := (p_input->>'duration_seconds')::numeric(10,2);
    v_import_metadata := p_input->'import_metadata';

    -- Insert the import record
    insert into dt_import_history (
        foundation_id,
        components_total,
        components_mapped,
        components_unmapped,
        projects_registered,
        duration_seconds,
        success_rate,
        import_metadata
    ) values (
        v_foundation_id,
        v_components_total,
        v_components_mapped,
        v_components_unmapped,
        v_projects_registered,
        v_duration_seconds,
        v_success_rate,
        v_import_metadata
    );
end
$$ language plpgsql;
