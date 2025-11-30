# DT Visibility Database Function Tests

This directory contains comprehensive pgTap tests for the DT (Dependency-Track) visibility database functions.

## Test Files

1. **record_dt_import.sql** (18 tests)
   - Tests the `record_dt_import(jsonb)` function
   - Validates import statistics recording
   - Tests success rate calculation
   - Validates error handling for missing required fields
   - Tests edge cases (zero components, optional fields)

2. **get_unmapped_components.sql** (17 tests)
   - Tests the `get_unmapped_components(jsonb)` function
   - Validates pagination (limit/offset)
   - Tests filtering by foundation_id
   - Tests search functionality (component_name, purl, component_group)
   - Validates data structure and field presence
   - Tests ordering (by last_seen desc)

3. **get_unmapped_stats.sql** (13 tests)
   - Tests the `get_unmapped_stats(jsonb)` function
   - Validates total count calculation
   - Tests by_foundation breakdown
   - Verifies ignored components are excluded
   - Tests filtering by foundation_id

## Total Test Coverage

- **Total Tests**: 48 comprehensive tests
- **Coverage**: All three DT visibility functions
- **Edge Cases**: Zero components, missing data, invalid inputs, pagination edge cases
- **Data Integrity**: Foreign key constraints, ignored flags, data structure validation

## Prerequisites

1. **PostgreSQL** with pgTap extension installed
2. **Database**: `clomonitor_tests`
3. **User**: `postgres` (or configured user with appropriate permissions)
4. **Migrations Applied**:
   - Schema: `012_add_dt_component_mapping.sql`, `013_add_dt_import_history.sql`
   - Functions: All DT functions in `database/migrations/functions/dt/`

## Running the Tests

### Run All DT Tests

```bash
cd clomonitor/database/tests
pg_prove --host localhost --dbname clomonitor_tests --username postgres --verbose functions/dt/*.sql
```

### Run Individual Test File

```bash
cd clomonitor/database/tests

# Test record_dt_import function
pg_prove --host localhost --dbname clomonitor_tests --username postgres --verbose functions/dt/record_dt_import.sql

# Test get_unmapped_components function
pg_prove --host localhost --dbname clomonitor_tests --username postgres --verbose functions/dt/get_unmapped_components.sql

# Test get_unmapped_stats function
pg_prove --host localhost --dbname clomonitor_tests --username postgres --verbose functions/dt/get_unmapped_stats.sql
```

### Alternative: Using clomonitor User

If you have a `clomonitor` database user configured:

```bash
pg_prove --host localhost --dbname clomonitor_tests --username clomonitor functions/dt/*.sql
```

## Test Database Setup

If you need to set up the test database:

```bash
# Create test database
createdb -U postgres clomonitor_tests

# Install pgTap extension
psql -U postgres -d clomonitor_tests -c "CREATE EXTENSION IF NOT EXISTS pgtap;"

# Apply migrations (from database/migrations directory)
cd clomonitor/database/migrations
TERN_CONF=~/.config/clomonitor/tern.conf ./migrate.sh
```

## Test Structure

Each test file follows this pattern:

```sql
BEGIN;                              -- Start transaction
SELECT plan(N);                     -- Declare number of tests

-- Setup test data (foundations, components, etc.)
INSERT INTO foundation (...) VALUES (...);
INSERT INTO dt_unmapped_components (...) VALUES (...);

-- Run tests
SELECT has_function(...);          -- Test function exists
SELECT is(...);                    -- Test equality
SELECT lives_ok(...);              -- Test no errors
SELECT throws_ok(...);             -- Test error handling
SELECT ok(...);                    -- Test boolean conditions

SELECT * FROM finish();            -- Complete tests
ROLLBACK;                          -- Cleanup (rollback all changes)
```

## What Each Test Validates

### record_dt_import.sql

- ✅ Function existence and signature
- ✅ Required fields validation (foundation_id, components_total, components_mapped, components_unmapped, projects_registered)
- ✅ Success rate calculation (mapped/total * 100)
- ✅ Optional fields handling (duration_seconds, import_metadata)
- ✅ Zero components edge case (no division by zero)
- ✅ Missing field error handling (proper exceptions)
- ✅ Data persistence in dt_import_history table

### get_unmapped_components.sql

- ✅ Function existence and return type
- ✅ Result structure (components array + total_count)
- ✅ Default behavior (returns all non-ignored components)
- ✅ Pagination (limit parameter)
- ✅ Offset handling (including offset beyond total)
- ✅ Foundation filtering
- ✅ Search functionality (case-insensitive, multi-field)
- ✅ Ignored components exclusion
- ✅ Ordering (last_seen descending)
- ✅ Component data structure completeness

### get_unmapped_stats.sql

- ✅ Function existence and return type
- ✅ Empty database handling (returns zeros)
- ✅ Total unmapped count accuracy
- ✅ by_foundation breakdown
- ✅ Ignored components exclusion
- ✅ Foundation filtering
- ✅ Data integrity (sum of by_foundation = total)
- ✅ Foundations with zero unmapped excluded from results
- ✅ Result structure validation

## Expected Output

When all tests pass, you should see:

```
functions/dt/record_dt_import.sql ........... ok
functions/dt/get_unmapped_components.sql .... ok
functions/dt/get_unmapped_stats.sql ......... ok
All tests successful.
Files=3, Tests=48
```

## Troubleshooting

### Test Failures

If tests fail, check:

1. **Migrations Applied**: Ensure all schema and function migrations are applied to `clomonitor_tests`
2. **pgTap Installed**: `SELECT * FROM pgtap_version();` should work
3. **Permissions**: User has CREATE/INSERT permissions on test database
4. **Function Definitions**: Functions match expected signatures

### Database Connection Issues

```bash
# Test connection
psql -U postgres -d clomonitor_tests -c "SELECT version();"

# Check if pgTap is installed
psql -U postgres -d clomonitor_tests -c "SELECT * FROM pgtap_version();"
```

### Verbose Output

For detailed test output:

```bash
pg_prove --host localhost --dbname clomonitor_tests --username postgres --verbose functions/dt/record_dt_import.sql
```

## Test Data

Tests use synthetic data that is:

- **Isolated**: Each test runs in a transaction that is rolled back
- **Realistic**: Uses realistic foundation IDs, component names, and purls
- **Complete**: Tests all code paths including edge cases
- **Independent**: Tests don't depend on each other

## CI Integration

These tests are designed to run in CI/CD pipelines:

```bash
# Exit on first failure
pg_prove --host localhost --dbname clomonitor_tests --username postgres functions/dt/*.sql
```

Exit code will be non-zero if any test fails.

## Maintenance

When modifying DT functions:

1. Update the corresponding function in `database/migrations/functions/dt/`
2. Update or add tests in this directory
3. Run all tests to ensure no regressions
4. Update test count in `SELECT plan(N);` if adding/removing tests
5. Document any new test scenarios in this README

## Related Documentation

- Function implementations: `clomonitor/database/migrations/functions/dt/`
- Schema migrations: `clomonitor/database/migrations/schema/`
- DT integration docs: `clomonitor/DT_VISIBILITY_IMPLEMENTATION_TRACKER.md`

## Contact

For issues with these tests, please refer to the CLOMonitor development documentation or create an issue in the project repository.
