-- Start transaction and plan tests
begin;
select plan(18);

-- Setup test data
insert into foundation (foundation_id, display_name, data_source_type, data_source_config)
values ('dt-test', 'DT Test Foundation', 'dependency_track', '{"dt_url": "https://test.example.com", "dt_api_key": "test"}'::jsonb)
on conflict do nothing;

-- Test 1: Function exists
select has_function(
    'record_dt_import',
    ARRAY['jsonb'],
    'Function record_dt_import(jsonb) should exist'
);

-- Test 2: Function returns void
select is(
    pg_typeof(record_dt_import('{"foundation_id": "dt-test", "components_total": 1, "components_mapped": 1, "components_unmapped": 0, "projects_registered": 1}'::jsonb))::text,
    'void',
    'Function should return void'
);

-- Test 3: Insert basic record with all required fields
select lives_ok(
    $$SELECT record_dt_import('{"foundation_id": "dt-test", "components_total": 100, "components_mapped": 75, "components_unmapped": 25, "projects_registered": 10}'::jsonb)$$,
    'record_dt_import succeeds with valid data'
);

-- Test 4: Verify record was inserted with correct total
select is(
    (SELECT components_total FROM dt_import_history WHERE foundation_id = 'dt-test' ORDER BY id DESC LIMIT 1),
    100,
    'Record inserted with correct components_total'
);

-- Test 5: Verify components_mapped
select is(
    (SELECT components_mapped FROM dt_import_history WHERE foundation_id = 'dt-test' ORDER BY id DESC LIMIT 1),
    75,
    'Record inserted with correct components_mapped'
);

-- Test 6: Verify components_unmapped
select is(
    (SELECT components_unmapped FROM dt_import_history WHERE foundation_id = 'dt-test' ORDER BY id DESC LIMIT 1),
    25,
    'Record inserted with correct components_unmapped'
);

-- Test 7: Verify projects_registered
select is(
    (SELECT projects_registered FROM dt_import_history WHERE foundation_id = 'dt-test' ORDER BY id DESC LIMIT 1),
    10,
    'Record inserted with correct projects_registered'
);

-- Test 8: Success rate calculation (75/100 = 75%)
select is(
    (SELECT success_rate FROM dt_import_history WHERE foundation_id = 'dt-test' ORDER BY id DESC LIMIT 1),
    75.00::numeric(5,2),
    'Success rate calculated correctly (75%)'
);

-- Test 9: Insert with optional fields (duration_seconds and import_metadata)
select lives_ok(
    $$SELECT record_dt_import('{"foundation_id": "dt-test", "components_total": 50, "components_mapped": 40, "components_unmapped": 10, "projects_registered": 5, "duration_seconds": 123.45, "import_metadata": {"source": "test", "version": "1.0"}}'::jsonb)$$,
    'record_dt_import succeeds with optional fields'
);

-- Test 10: Verify duration_seconds
select is(
    (SELECT duration_seconds FROM dt_import_history WHERE foundation_id = 'dt-test' ORDER BY id DESC LIMIT 1),
    123.45::numeric(10,2),
    'Optional duration_seconds stored correctly'
);

-- Test 11: Verify import_metadata
select is(
    (SELECT import_metadata FROM dt_import_history WHERE foundation_id = 'dt-test' ORDER BY id DESC LIMIT 1),
    '{"source": "test", "version": "1.0"}'::jsonb,
    'Optional import_metadata stored correctly'
);

-- Test 12: Zero components edge case (no division by zero)
select lives_ok(
    $$SELECT record_dt_import('{"foundation_id": "dt-test", "components_total": 0, "components_mapped": 0, "components_unmapped": 0, "projects_registered": 0}'::jsonb)$$,
    'Handles zero components without error'
);

-- Test 13: Verify success_rate is 0 when total is 0
select is(
    (SELECT success_rate FROM dt_import_history WHERE foundation_id = 'dt-test' ORDER BY id DESC LIMIT 1),
    0.00::numeric(5,2),
    'Success rate is 0 when components_total is 0'
);

-- Test 14: Missing foundation_id should raise exception
select throws_ok(
    $$SELECT record_dt_import('{"components_total": 100, "components_mapped": 75, "components_unmapped": 25, "projects_registered": 10}'::jsonb)$$,
    'foundation_id is required',
    'Missing foundation_id throws exception'
);

-- Test 15: Missing components_total should raise exception
select throws_ok(
    $$SELECT record_dt_import('{"foundation_id": "dt-test", "components_mapped": 75, "components_unmapped": 25, "projects_registered": 10}'::jsonb)$$,
    'components_total is required',
    'Missing components_total throws exception'
);

-- Test 16: Missing components_mapped should raise exception
select throws_ok(
    $$SELECT record_dt_import('{"foundation_id": "dt-test", "components_total": 100, "components_unmapped": 25, "projects_registered": 10}'::jsonb)$$,
    'components_mapped is required',
    'Missing components_mapped throws exception'
);

-- Test 17: Missing components_unmapped should raise exception
select throws_ok(
    $$SELECT record_dt_import('{"foundation_id": "dt-test", "components_total": 100, "components_mapped": 75, "projects_registered": 10}'::jsonb)$$,
    'components_unmapped is required',
    'Missing components_unmapped throws exception'
);

-- Test 18: Missing projects_registered should raise exception
select throws_ok(
    $$SELECT record_dt_import('{"foundation_id": "dt-test", "components_total": 100, "components_mapped": 75, "components_unmapped": 25}'::jsonb)$$,
    'projects_registered is required',
    'Missing projects_registered throws exception'
);

-- Finish tests and rollback transaction
select * from finish();
rollback;
