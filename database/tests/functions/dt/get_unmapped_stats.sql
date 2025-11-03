-- Start transaction and plan tests
begin;
select plan(13);

-- Setup test data
insert into foundation (foundation_id, display_name, data_source_type, data_source_config)
values
    ('dt-test-1', 'DT Test Foundation 1', 'dependency_track', '{"dt_url": "https://test1.example.com"}'::jsonb),
    ('dt-test-2', 'DT Test Foundation 2', 'dependency_track', '{"dt_url": "https://test2.example.com"}'::jsonb),
    ('dt-test-3', 'DT Test Foundation 3', 'dependency_track', '{"dt_url": "https://test3.example.com"}'::jsonb)
on conflict do nothing;

-- Test 1: Function exists
select has_function(
    'get_unmapped_stats',
    ARRAY['jsonb'],
    'Function get_unmapped_stats(jsonb) should exist'
);

-- Test 2: Function returns jsonb
select is(
    pg_typeof(get_unmapped_stats('{}'::jsonb))::text,
    'jsonb',
    'Function should return jsonb'
);

-- Test 3: Empty database returns zero counts
select is(
    get_unmapped_stats('{}'::jsonb),
    '{"total_unmapped": 0, "by_foundation": {}}'::jsonb,
    'Empty database returns 0 total_unmapped and empty by_foundation'
);

-- Insert test unmapped components
insert into dt_unmapped_components (
    foundation_id,
    component_uuid,
    component_name,
    component_version,
    purl,
    classifier,
    mapping_attempts,
    first_seen,
    last_seen,
    ignored
) values
    -- Foundation 1: 3 unmapped, 1 ignored
    ('dt-test-1', 'uuid-1', 'spring-boot', '2.5.0', 'pkg:maven/org.springframework.boot/spring-boot@2.5.0', 'LIBRARY', 0, '2025-01-01 10:00:00+00', '2025-01-05 10:00:00+00', false),
    ('dt-test-1', 'uuid-2', 'react', '18.0.0', 'pkg:npm/react@18.0.0', 'LIBRARY', 1, '2025-01-02 10:00:00+00', '2025-01-04 10:00:00+00', false),
    ('dt-test-1', 'uuid-3', 'lodash', '4.17.21', 'pkg:npm/lodash@4.17.21', 'LIBRARY', 2, '2025-01-03 10:00:00+00', '2025-01-03 10:00:00+00', false),
    ('dt-test-1', 'uuid-4', 'ignored-lib', '1.0.0', 'pkg:npm/ignored-lib@1.0.0', 'LIBRARY', 0, '2025-01-04 10:00:00+00', '2025-01-02 10:00:00+00', true),
    -- Foundation 2: 2 unmapped
    ('dt-test-2', 'uuid-5', 'django', '4.0.0', 'pkg:pypi/django@4.0.0', 'LIBRARY', 0, '2025-01-01 10:00:00+00', '2025-01-01 10:00:00+00', false),
    ('dt-test-2', 'uuid-6', 'flask', '2.0.0', 'pkg:pypi/flask@2.0.0', 'LIBRARY', 0, '2025-01-02 10:00:00+00', '2025-01-01 10:00:00+00', false),
    -- Foundation 3: 0 unmapped (all ignored)
    ('dt-test-3', 'uuid-7', 'all-ignored', '1.0.0', 'pkg:npm/all-ignored@1.0.0', 'LIBRARY', 0, '2025-01-01 10:00:00+00', '2025-01-01 10:00:00+00', true);

-- Test 4: Total unmapped count correct (excludes ignored)
select is(
    (get_unmapped_stats('{}'::jsonb)->>'total_unmapped')::int,
    5,
    'Total unmapped is 5 (excludes 2 ignored components)'
);

-- Test 5: by_foundation breakdown correct
select is(
    get_unmapped_stats('{}'::jsonb)->'by_foundation',
    '{"dt-test-1": 3, "dt-test-2": 2}'::jsonb,
    'by_foundation shows correct breakdown (excludes foundation with 0 unmapped)'
);

-- Test 6: Sum of by_foundation equals total_unmapped
select is(
    ((get_unmapped_stats('{}'::jsonb)->'by_foundation'->>'dt-test-1')::int +
     (get_unmapped_stats('{}'::jsonb)->'by_foundation'->>'dt-test-2')::int),
    (get_unmapped_stats('{}'::jsonb)->>'total_unmapped')::int,
    'Sum of by_foundation counts equals total_unmapped'
);

-- Test 7: Foundation with only ignored components not included in by_foundation
select ok(
    not (get_unmapped_stats('{}'::jsonb)->'by_foundation' ? 'dt-test-3'),
    'Foundation with only ignored components not in by_foundation'
);

-- Test 8: Filter by specific foundation_id
select is(
    get_unmapped_stats('{"foundation_id": "dt-test-1"}'::jsonb),
    '{"total_unmapped": 3, "by_foundation": {"dt-test-1": 3}}'::jsonb,
    'Filter by foundation_id returns stats for that foundation only'
);

-- Test 9: Filter by foundation with no unmapped components
select is(
    get_unmapped_stats('{"foundation_id": "dt-test-3"}'::jsonb),
    '{"total_unmapped": 0, "by_foundation": {}}'::jsonb,
    'Foundation with only ignored components returns 0 stats'
);

-- Test 10: Filter by non-existent foundation
select is(
    get_unmapped_stats('{"foundation_id": "nonexistent"}'::jsonb),
    '{"total_unmapped": 0, "by_foundation": {}}'::jsonb,
    'Non-existent foundation returns 0 stats'
);

-- Test 11: Result structure is correct
select ok(
    (get_unmapped_stats('{}'::jsonb) ? 'total_unmapped') and
    (get_unmapped_stats('{}'::jsonb) ? 'by_foundation'),
    'Result contains total_unmapped and by_foundation keys'
);

-- Test 12: total_unmapped is an integer
select is(
    jsonb_typeof(get_unmapped_stats('{}'::jsonb)->'total_unmapped'),
    'number',
    'total_unmapped is a number'
);

-- Test 13: by_foundation is an object
select is(
    jsonb_typeof(get_unmapped_stats('{}'::jsonb)->'by_foundation'),
    'object',
    'by_foundation is an object'
);

-- Finish tests and rollback transaction
select * from finish();
rollback;
