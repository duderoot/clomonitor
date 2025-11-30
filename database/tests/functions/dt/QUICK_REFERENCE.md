# DT Tests Quick Reference Card

## Run All Tests
```bash
cd clomonitor/database/tests
pg_prove --host localhost --dbname clomonitor_tests --username postgres --verbose functions/dt/*.sql
```

## Run Individual Tests
```bash
# Test record_dt_import
pg_prove --host localhost --dbname clomonitor_tests --username postgres functions/dt/record_dt_import.sql

# Test get_unmapped_components
pg_prove --host localhost --dbname clomonitor_tests --username postgres functions/dt/get_unmapped_components.sql

# Test get_unmapped_stats
pg_prove --host localhost --dbname clomonitor_tests --username postgres functions/dt/get_unmapped_stats.sql
```

## Test Summary

| Test File | Function | Tests | Focus |
|-----------|----------|-------|-------|
| `record_dt_import.sql` | `record_dt_import(jsonb)` | 18 | Import stats, success rate, error handling |
| `get_unmapped_components.sql` | `get_unmapped_components(jsonb)` | 17 | Pagination, filtering, search |
| `get_unmapped_stats.sql` | `get_unmapped_stats(jsonb)` | 13 | Counting, aggregation, filtering |
| **TOTAL** | **3 functions** | **48** | **100% coverage** |

## Key Test Categories

### record_dt_import (18 tests)
- ✅ Function metadata (2)
- ✅ Required fields validation (5)
- ✅ Data persistence (4)
- ✅ Optional fields (2)
- ✅ Edge cases (2)
- ✅ Error handling (5)

### get_unmapped_components (17 tests)
- ✅ Function metadata (2)
- ✅ Result structure (2)
- ✅ Pagination (4)
- ✅ Filtering (3)
- ✅ Search (3)
- ✅ Data integrity (3)

### get_unmapped_stats (13 tests)
- ✅ Function metadata (2)
- ✅ Empty state (1)
- ✅ Counting (4)
- ✅ Filtering (3)
- ✅ Data integrity (3)

## Expected Success Output
```
All tests successful.
Files=3, Tests=48, ~2 seconds
```

## Prerequisites
- PostgreSQL with pgTap extension
- Database: `clomonitor_tests`
- User: `postgres` (or `clomonitor`)
- Migrations applied: schemas 012-013, all DT functions

## Test Database Setup (if needed)
```bash
createdb -U postgres clomonitor_tests
psql -U postgres -d clomonitor_tests -c "CREATE EXTENSION pgtap;"
cd clomonitor/database/migrations
TERN_CONF=~/.config/clomonitor/tern.conf ./migrate.sh
```

## Troubleshooting

### "function does not exist"
→ Apply DT function migrations first

### "relation does not exist"
→ Apply schema migrations 012 and 013

### "could not connect to database"
→ Check PostgreSQL is running and test DB exists

## Documentation
- Full README: `README.md`
- Coverage Report: `TEST_COVERAGE_REPORT.md`
- This Quick Reference: `QUICK_REFERENCE.md`
