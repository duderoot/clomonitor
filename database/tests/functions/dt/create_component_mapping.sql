-- Start transaction and plan tests
begin;
select plan(15);

-- Test 1: Function exists
select has_function(
    'create_component_mapping',
    ARRAY['jsonb'],
    'Function create_component_mapping(jsonb) should exist'
);

-- Test 2: Function returns jsonb
select is(
    pg_typeof(create_component_mapping('{"purl": "pkg:npm/test@1.0.0", "repository_url": "https://github.com/test/test"}'::jsonb))::text,
    'jsonb',
    'Function should return jsonb'
);

-- Test 3: Create mapping with all required fields (minimal)
select ok(
    (create_component_mapping('{"purl": "pkg:npm/test-lib@1.0.0", "repository_url": "https://github.com/example/test-lib"}'::jsonb) ? 'purl'),
    'Create mapping with minimal required fields succeeds'
);

-- Test 4: Verify purl is stored correctly
select is(
    (create_component_mapping('{"purl": "pkg:maven/org.example/library@2.0.0", "repository_url": "https://github.com/example/library"}'::jsonb)->>'purl'),
    'pkg:maven/org.example/library@2.0.0',
    'Purl stored correctly in created mapping'
);

-- Test 5: Verify repository_url is stored correctly
select is(
    (create_component_mapping('{"purl": "pkg:pypi/django@4.0.0", "repository_url": "https://github.com/django/django"}'::jsonb)->>'repository_url'),
    'https://github.com/django/django',
    'Repository URL stored correctly'
);

-- Test 6: Create mapping with optional fields (notes, created_by)
select ok(
    (create_component_mapping('{"purl": "pkg:npm/react@18.0.0", "repository_url": "https://github.com/facebook/react", "notes": "Official React repository", "created_by": "admin@example.com"}'::jsonb)->>'notes') = 'Official React repository',
    'Create mapping with optional notes field succeeds'
);

-- Test 7: Verify created_by is stored
select is(
    (create_component_mapping('{"purl": "pkg:npm/lodash@4.17.21", "repository_url": "https://github.com/lodash/lodash", "created_by": "user@test.com"}'::jsonb)->>'created_by'),
    'user@test.com',
    'created_by field stored correctly'
);

-- Test 8: Default mapping_source is 'manual'
select is(
    (create_component_mapping('{"purl": "pkg:npm/axios@1.0.0", "repository_url": "https://github.com/axios/axios"}'::jsonb)->>'mapping_source'),
    'manual',
    'Default mapping_source is manual when not provided'
);

-- Test 9: Custom mapping_source can be specified
select is(
    (create_component_mapping('{"purl": "pkg:npm/webpack@5.0.0", "repository_url": "https://github.com/webpack/webpack", "mapping_source": "automatic"}'::jsonb)->>'mapping_source'),
    'automatic',
    'Custom mapping_source can be specified'
);

-- Test 10: Confidence score validation and storage
select is(
    (create_component_mapping('{"purl": "pkg:npm/typescript@5.0.0", "repository_url": "https://github.com/microsoft/typescript", "confidence_score": 95}'::jsonb)->>'confidence_score'),
    '95',
    'Confidence score stored correctly'
);

-- Test 11: Verified flag defaults to false
select is(
    (create_component_mapping('{"purl": "pkg:npm/express@4.18.0", "repository_url": "https://github.com/expressjs/express"}'::jsonb)->>'verified'),
    'false',
    'Verified defaults to false when not provided'
);

-- Test 12: Verified flag can be set to true
select is(
    (create_component_mapping('{"purl": "pkg:npm/vue@3.0.0", "repository_url": "https://github.com/vuejs/vue", "verified": true}'::jsonb)->>'verified'),
    'true',
    'Verified can be set to true'
);

-- Test 13: Missing purl should raise exception
select throws_ok(
    $$SELECT create_component_mapping('{"repository_url": "https://github.com/test/test"}'::jsonb)$$,
    'purl is required and cannot be empty',
    'Missing purl throws exception'
);

-- Test 14: Missing repository_url should raise exception
select throws_ok(
    $$SELECT create_component_mapping('{"purl": "pkg:npm/test@1.0.0"}'::jsonb)$$,
    'repository_url is required and cannot be empty',
    'Missing repository_url throws exception'
);

-- Test 15: Empty purl should raise exception
select throws_ok(
    $$SELECT create_component_mapping('{"purl": "", "repository_url": "https://github.com/test/test"}'::jsonb)$$,
    'purl is required and cannot be empty',
    'Empty purl throws exception'
);

-- Finish tests and rollback transaction
select * from finish();
rollback;
