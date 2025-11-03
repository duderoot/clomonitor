-- Start transaction and plan tests
begin;
select plan(17);

-- Setup test data
insert into foundation (foundation_id, display_name, data_source_type, data_source_config)
values
    ('dt-test-1', 'DT Test Foundation 1', 'dependency_track', '{"dt_url": "https://test1.example.com"}'::jsonb),
    ('dt-test-2', 'DT Test Foundation 2', 'dependency_track', '{"dt_url": "https://test2.example.com"}'::jsonb)
on conflict do nothing;

-- Insert test unmapped components
insert into dt_unmapped_components (
    foundation_id,
    component_uuid,
    component_name,
    component_version,
    component_group,
    purl,
    classifier,
    mapping_attempts,
    first_seen,
    last_seen,
    mapping_notes,
    external_references,
    ignored
) values
    -- Foundation 1 components
    ('dt-test-1', 'uuid-1', 'spring-boot', '2.5.0', 'org.springframework.boot', 'pkg:maven/org.springframework.boot/spring-boot@2.5.0', 'LIBRARY', 0, '2025-01-01 10:00:00+00', '2025-01-05 10:00:00+00', 'Needs manual mapping', '{"github": "https://github.com/spring-projects/spring-boot"}'::jsonb, false),
    ('dt-test-1', 'uuid-2', 'react', '18.0.0', 'facebook', 'pkg:npm/react@18.0.0', 'LIBRARY', 1, '2025-01-02 10:00:00+00', '2025-01-04 10:00:00+00', null, null, false),
    ('dt-test-1', 'uuid-3', 'lodash', '4.17.21', null, 'pkg:npm/lodash@4.17.21', 'LIBRARY', 2, '2025-01-03 10:00:00+00', '2025-01-03 10:00:00+00', null, null, false),
    ('dt-test-1', 'uuid-4', 'ignored-lib', '1.0.0', null, 'pkg:npm/ignored-lib@1.0.0', 'LIBRARY', 0, '2025-01-04 10:00:00+00', '2025-01-02 10:00:00+00', 'Ignored component', null, true),
    -- Foundation 2 components
    ('dt-test-2', 'uuid-5', 'django', '4.0.0', 'django', 'pkg:pypi/django@4.0.0', 'LIBRARY', 0, '2025-01-01 10:00:00+00', '2025-01-01 10:00:00+00', null, null, false),
    ('dt-test-2', 'uuid-6', 'flask', '2.0.0', 'pallets', 'pkg:pypi/flask@2.0.0', 'LIBRARY', 0, '2025-01-02 10:00:00+00', '2025-01-01 10:00:00+00', null, null, false);

-- Test 1: Function exists
select has_function(
    'get_unmapped_components',
    ARRAY['jsonb'],
    'Function get_unmapped_components(jsonb) should exist'
);

-- Test 2: Function returns jsonb
select is(
    pg_typeof(get_unmapped_components('{}'::jsonb))::text,
    'jsonb',
    'Function should return jsonb'
);

-- Test 3: Returns object with components array and total_count
select ok(
    (get_unmapped_components('{}'::jsonb) ? 'components') and
    (get_unmapped_components('{}'::jsonb) ? 'total_count'),
    'Result contains components and total_count keys'
);

-- Test 4: Default behavior returns all non-ignored components
select is(
    (get_unmapped_components('{}'::jsonb)->>'total_count')::int,
    5,
    'Total count excludes ignored components (5 total, 1 ignored)'
);

-- Test 5: Default limit is 20, should return all 5 components
select is(
    jsonb_array_length(get_unmapped_components('{}'::jsonb)->'components'),
    5,
    'Default behavior returns all 5 non-ignored components'
);

-- Test 6: Filter by foundation_id
select is(
    (get_unmapped_components('{"foundation_id": "dt-test-1"}'::jsonb)->>'total_count')::int,
    3,
    'Filter by foundation_id returns correct count (3 for dt-test-1)'
);

-- Test 7: Verify foundation filter returns correct components
select is(
    jsonb_array_length(get_unmapped_components('{"foundation_id": "dt-test-1"}'::jsonb)->'components'),
    3,
    'Filter by foundation_id returns correct number of components'
);

-- Test 8: Limit parameter works
select is(
    jsonb_array_length(get_unmapped_components('{"limit": 2}'::jsonb)->'components'),
    2,
    'Limit parameter restricts results to 2 components'
);

-- Test 9: Total count accurate regardless of limit
select is(
    (get_unmapped_components('{"limit": 2}'::jsonb)->>'total_count')::int,
    5,
    'Total count remains accurate with limit'
);

-- Test 10: Offset parameter works
select is(
    jsonb_array_length(get_unmapped_components('{"limit": 2, "offset": 2}'::jsonb)->'components'),
    2,
    'Offset parameter works correctly'
);

-- Test 11: Offset beyond total returns empty array
select is(
    jsonb_array_length(get_unmapped_components('{"offset": 100}'::jsonb)->'components'),
    0,
    'Offset beyond total returns empty array'
);

-- Test 12: Search by component_name (case-insensitive)
select is(
    (get_unmapped_components('{"search": "spring"}'::jsonb)->>'total_count')::int,
    1,
    'Search by component_name finds spring-boot'
);

-- Test 13: Search by purl
select is(
    (get_unmapped_components('{"search": "npm/react"}'::jsonb)->>'total_count')::int,
    1,
    'Search by purl finds react'
);

-- Test 14: Search by component_group
select is(
    (get_unmapped_components('{"search": "facebook"}'::jsonb)->>'total_count')::int,
    1,
    'Search by component_group finds facebook/react'
);

-- Test 15: Search with no matches
select is(
    (get_unmapped_components('{"search": "nonexistent"}'::jsonb)->>'total_count')::int,
    0,
    'Search with no matches returns 0 count'
);

-- Test 16: Component structure has required fields
select ok(
    (get_unmapped_components('{"limit": 1}'::jsonb)->'components'->0 ? 'id') and
    (get_unmapped_components('{"limit": 1}'::jsonb)->'components'->0 ? 'uuid') and
    (get_unmapped_components('{"limit": 1}'::jsonb)->'components'->0 ? 'foundation_id') and
    (get_unmapped_components('{"limit": 1}'::jsonb)->'components'->0 ? 'component_name') and
    (get_unmapped_components('{"limit": 1}'::jsonb)->'components'->0 ? 'purl') and
    (get_unmapped_components('{"limit": 1}'::jsonb)->'components'->0 ? 'first_seen') and
    (get_unmapped_components('{"limit": 1}'::jsonb)->'components'->0 ? 'last_seen_at') and
    (get_unmapped_components('{"limit": 1}'::jsonb)->'components'->0 ? 'mapping_attempts'),
    'Component object contains all required fields'
);

-- Test 17: Components ordered by last_seen desc (most recent first)
select is(
    (get_unmapped_components('{"foundation_id": "dt-test-1", "limit": 1}'::jsonb)->'components'->0->>'component_name'),
    'spring-boot',
    'Components ordered by last_seen descending (spring-boot most recent)'
);

-- Finish tests and rollback transaction
select * from finish();
rollback;
