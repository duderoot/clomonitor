-- ignore_unmapped_component marks an unmapped component as ignored or unignored
-- Input: {
--   "component_id": 123,  -- required, dt_unmapped_components.id
--   "ignored": true,  -- required, true to ignore, false to unignore
--   "ignore_reason": "internal_library",  -- optional, reason for ignoring
--   "ignored_by": "user@example.com",  -- optional, who ignored it
--   "notes": "Internal library, not for tracking"  -- optional, additional notes
-- }
-- Returns: updated component record
create or replace function ignore_unmapped_component(p_input jsonb)
returns jsonb as $$
declare
    v_component_id int;
    v_ignored boolean;
    v_ignore_reason text;
    v_ignored_by text;
    v_notes text;
    v_result jsonb;
    v_exists boolean;
begin
    -- Extract and validate required fields
    v_component_id := (p_input->>'component_id')::int;
    if v_component_id is null then
        raise exception 'component_id is required';
    end if;

    if not (p_input ? 'ignored') then
        raise exception 'ignored field is required (true or false)';
    end if;
    v_ignored := (p_input->>'ignored')::boolean;

    -- Check if component exists
    select exists(select 1 from dt_unmapped_components where id = v_component_id) into v_exists;
    if not v_exists then
        raise exception 'Unmapped component with id % not found', v_component_id;
    end if;

    -- Extract optional fields
    v_ignore_reason := p_input->>'ignore_reason';
    v_ignored_by := p_input->>'ignored_by';
    v_notes := p_input->>'notes';

    -- Update the component's ignored status
    update dt_unmapped_components
    set
        ignored = v_ignored,
        ignore_reason = case
            when v_ignored then coalesce(v_ignore_reason, ignore_reason)
            else null  -- Clear reason when unignoring
        end,
        ignored_by = case
            when v_ignored then coalesce(v_ignored_by, ignored_by)
            else null  -- Clear who ignored when unignoring
        end,
        ignored_at = case
            when v_ignored then now()
            else null  -- Clear timestamp when unignoring
        end,
        mapping_notes = case
            when v_notes is not null then v_notes
            else mapping_notes
        end,
        last_seen = now()  -- Update last_seen to track when it was modified
    where id = v_component_id
    returning jsonb_build_object(
        'id', id,
        'foundation_id', foundation_id,
        'component_uuid', component_uuid,
        'component_name', component_name,
        'component_version', component_version,
        'component_group', component_group,
        'purl', purl,
        'classifier', classifier,
        'ignored', ignored,
        'ignore_reason', ignore_reason,
        'ignored_by', ignored_by,
        'ignored_at', case
            when ignored_at is not null then to_char(ignored_at at time zone 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"')
            else null
        end,
        'mapping_notes', mapping_notes,
        'mapping_attempts', mapping_attempts,
        'first_seen', to_char(first_seen at time zone 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
        'last_seen', to_char(last_seen at time zone 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
        'external_references', external_references
    ) into v_result;

    return v_result;
end;
$$ language plpgsql;
