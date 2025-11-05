# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

`clomonitor-registrar` is a Rust backend component that registers projects from foundation data files (YAML) into the CLOMonitor PostgreSQL database. It is part of the larger CLOMonitor workspace, which monitors open source project health across CNCF, LF AI & DATA, and CDF foundations.

## Build and Test Commands

```bash
# Build (from workspace root or this directory)
cargo build

# Run tests
cargo test

# Run with config file (requires PostgreSQL)
cargo run -- --config ~/.config/clomonitor/registrar.yaml

# Debug build location
./target/debug/clomonitor-registrar -c <config-file>
```

## Configuration

The registrar requires a YAML config file with database connection details and concurrency settings:

```yaml
db:
  host: localhost
  port: "5432"
  dbname: clomonitor
  user: postgres
  password: ""
registrar:
  concurrency: 1
```

## Architecture

### Code Structure

- `src/main.rs`: Entry point, sets up config, logging, database pool with SSL, and runs the registrar
- `src/registrar.rs`: Core logic for processing foundations and their projects
- `src/db.rs`: Database trait and PostgreSQL implementation using database functions

### Key Concepts

**Foundations**: Organizations (CNCF, LF AI & DATA, CDF) that maintain data files listing their projects. Each foundation has a `data_url` pointing to a YAML file served over HTTP.

**Projects**: Defined in foundation YAML files with metadata (name, description, maturity, etc.) and one or more repositories. Each project gets a digest (SHA256 hash) to detect changes.

**Repositories**: Individual repos within projects that have URLs, optional `check_sets` (code, code-lite, community, docs), and optional `exclude` lists.

### Processing Flow

1. Fetch all foundations from database (`foundations()`)
2. For each foundation (processed concurrently):
   - Fetch YAML data file from `data_url` via HTTP
   - Parse YAML into `Vec<Project>`
   - Filter out repositories with `exclude: ["clomonitor"]`
   - Calculate digest for each project
   - Get currently registered projects from database (`foundation_projects()`)
   - Register new/changed projects (`register_project()`)
   - Unregister projects removed from data file (`unregister_project()`)

### Database Layer

The registrar calls PostgreSQL functions, not raw SQL:
- `register_project($1::text, $2::jsonb)`: Upserts project data
- `unregister_project($1::text, $2::text)`: Removes project

Database functions are defined in `database/migrations/functions/` and tested in `database/tests/functions/` (using pgTap).

### Testing

Tests use `mockall` for mocking the DB trait and `mockito` for HTTP mocking. Test data is in `src/testdata/cncf.yaml`. Tests verify:
- Error handling (DB errors, HTTP errors, invalid YAML)
- Project registration logic (digest comparison, register/unregister)
- Concurrency handling and timeout (300s per foundation)

## Workspace Context

This is one component in a Cargo workspace at `../`:
- `clomonitor-core`: Shared linting and scoring logic
- `clomonitor-apiserver`: HTTP API and web serving
- `clomonitor-tracker`: Lints repositories periodically
- `clomonitor-archiver`: Creates project data snapshots
- `clomonitor-linter`: CLI tool for local linting

All backend components use the same database (schema in `database/migrations/schema/`, functions in `database/migrations/functions/`). Migrations are managed with Tern.

## Development Notes

- Uses `async-trait` for database trait
- Requires PostgreSQL with TLS (uses `openssl` with `SslVerifyMode::NONE`)
- Logging configured via `RUST_LOG` env var (default: `clomonitor_registrar=debug`)
- Supports JSON log format via config: `log.format: "json"`
- Foundation processing has 300-second timeout per foundation
- Uses `deadpool-postgres` for connection pooling
- Projects are serialized to JSON using `serde_json` when calling DB functions

## Resilience and Error Handling

### Incremental Registration Strategy

The DT processing implementation uses an **incremental registration** pattern for resilience:

**Key Principles:**
- Each component is registered immediately after successful mapping (no batching)
- Progress is committed to the database continuously, not at the end
- Component-level failures don't stop processing of other components
- Project-level failures don't stop processing of other projects
- Restarts are idempotent via digest-based deduplication

**Benefits:**
- 99% of work preserved on failure vs 0% with batch processing
- Minimal memory usage (no accumulation of large datasets in memory)
- Fast recovery from failures (only failed items need reprocessing)
- Detailed error tracking with granular failure statistics

### Error Handling Patterns

#### Process DT Foundation (`process_dt_foundation`)
- **DT API failure**: Entire foundation fails (Err propagated)
- **Project-level errors**: Logged, tracked in stats, other projects continue
- **Cleanup errors**: Logged but don't fail the run

#### Process Single DT Project (`process_single_dt_project`)
- **Component fetch failure**: Project fails (Err propagated)
- **Component mapping failure**: Component saved as unmapped, others continue
- **Registration failure**: Logged, tracked, others continue
- **Unmapped save failure**: Logged, tracked, others continue

Returns `ProcessingResult` with detailed counts:
- `mapped_count`: Successfully registered components
- `unmapped_count`: Components without repo URLs
- `failed_registrations`: Registration errors
- `failed_unmapped_saves`: Database errors saving unmapped components

#### Register Project If Changed (`register_project_if_changed`)
- **Database query error**: Propagated as Err
- **Registration error**: Propagated as Err
- Implements digest-based deduplication for idempotent restarts

#### Cleanup Removed Projects (`cleanup_removed_projects`)
- **Query error**: Propagated as Err
- **Individual unregister errors**: Logged but don't stop cleanup of other projects

### Logging and Observability

**Structured logging levels:**
- `debug`: Per-component/per-project progress details
- `info`: Foundation-level summary statistics
- `error`: Component/project failures with full context

**Key metrics logged:**
- Projects processed
- Components mapped (successfully registered)
- Components unmapped (no repo URL found)
- Registration failures (database/project build errors)
- Unmapped save failures (database errors)
- Project-level errors (component fetch failures)

**Example log output:**
```
INFO DT foundation my-dt: 100 projects processed, 850 components mapped, 120 unmapped, 5 registration failures, 2 unmapped save failures, 3 project errors
```

### Testing Resilience

**Failure scenarios tested:**
1. Empty DT project list (no-op, no errors)
2. DT project with no components (no-op, no errors)
3. All components filtered out (only LIBRARY/FRAMEWORK processed)
4. Components without repo URLs (saved as unmapped)
5. Non-library components (CONTAINER, APPLICATION skipped)

**Rate limiting:**
- DT API: 429 responses handled with retry and exponential backoff (max 3 retries)
- Registry APIs: Best effort, errors logged but don't stop processing

**Database resilience:**
- Connection pooling via `deadpool-postgres`
- Per-operation error handling (failures don't cascade)
- Digest-based deduplication prevents redundant writes on restart

### Troubleshooting

**Problem**: High failure counts in logs

**Solution**: Check error logs for specific failures:
```bash
RUST_LOG=clomonitor_registrar=error cargo run -- --config config.yaml 2>&1 | grep "Failed to"
```

**Problem**: DT processing slow or timing out

**Solution**:
- Increase `FOUNDATION_TIMEOUT` in code (default 300s)
- Check DT API rate limiting (429 responses in logs)
- Verify network connectivity to DT instance

**Problem**: Components not registered

**Possible causes**:
- Component classifier not LIBRARY/FRAMEWORK (check debug logs for "Skipping component")
- No repository URL found (check unmapped components table)
- Registration failures (check error logs for "Failed to register")

**Problem**: Memory usage high

**Resolution**: The incremental registration pattern should keep memory usage low. If high:
- Check for memory leaks in DT client
- Verify components are being processed incrementally (not accumulated)
- Monitor with: `RUST_LOG=debug` and look for batch processing patterns
