# Quick Start - Phase 1 Resilience Implementation

## TDD Red Phase ✅ COMPLETE

### What Was Done
- ✅ Designed 15 comprehensive resilience tests
- ✅ Documented test specifications
- ✅ Verified baseline (77 tests passing)
- ✅ Created implementation roadmap

### Deliverables
1. **`RESILIENCE_TEST_REPORT.md`** - Detailed test specifications (READ THIS FIRST)
2. **`RESILIENCE_TDD_SUMMARY.md`** - Executive summary
3. **`QUICK_START_RESILIENCE.md`** - This file

---

## Quick Implementation Guide

### The Problem (Current Code)
```rust
// ❌ BATCH EVERYTHING - All-or-nothing design
let mut all_projects = HashMap::new();
for dt_project in dt_projects {
    // Fetch components
    let components = dt_client.get_components(&dt_project.uuid).await?; // STOPS ON ERROR

    // Process components
    for component in components {
        all_projects.insert(name, project); // MEMORY ONLY
    }
}

// Register at the END (too late!)
for project in all_projects {
    db.register_project(...).await?; // LATE REGISTRATION
}
```

**Problems**:
1. Fail at project #999 of 1000 → LOSE ALL 999 projects
2. All data in memory until end → MEMORY GROWS
3. No progress until complete → RESTART FROM ZERO

### The Solution (Phase 1)
```rust
// ✅ INCREMENTAL - Resilient design
for dt_project in dt_projects {
    // Process EACH project independently
    match process_single_dt_project(&db, &dt_client, &dt_project, ...).await {
        Ok(result) => {
            // Project already registered immediately inside function
            registered_names.extend(result.registered_names);
        }
        Err(e) => {
            error!("Failed project {}: {}", dt_project.name, e);
            continue; // ✅ KEEP GOING
        }
    }
}
```

**Benefits**:
1. Fail at project #999 → KEEP 998 registered projects ✅
2. Register immediately → LOW MEMORY ✅
3. Incremental progress → RESTART ONLY FAILED ✅

---

## Implementation Steps (3 hours)

### Step 1: Add Helper Structs (15 min)
```rust
struct ProcessingResult {
    dt_project_name: String,
    dt_project_uuid: String,
    registered_names: Vec<String>,
    mapped_count: usize,
    unmapped_count: usize,
}

#[derive(Default)]
struct ProcessingStats {
    projects_processed: usize,
    components_mapped: usize,
    components_unmapped: usize,
    errors: Vec<String>,
}

impl ProcessingStats {
    fn merge(&mut self, result: &ProcessingResult) {
        self.projects_processed += 1;
        self.components_mapped += result.mapped_count;
        self.components_unmapped += result.unmapped_count;
    }
}
```

### Step 2: Add Digest Helper (15 min)
```rust
async fn register_project_if_changed(
    db: &DynDB,
    foundation_id: &str,
    project: &Project,
) -> Result<bool> {
    // Check if project already exists with same digest
    let existing_projects = db.foundation_projects(foundation_id).await?;

    if let Some(registered_digest) = existing_projects.get(&project.name) {
        if registered_digest == &project.digest {
            // Project unchanged, skip registration
            return Ok(false);
        }
    }

    // Project is new or changed, register it
    db.register_project(foundation_id, project).await?;
    Ok(true)
}
```

### Step 3: Extract Single Project Processing (30 min)
```rust
async fn process_single_dt_project(
    db: &DynDB,
    dt_client: &DtHttpClient,
    dt_project: &DtProject,
    foundation_id: &str,
    registry_router: Option<&Arc<RegistryRouter>>,
) -> Result<ProcessingResult> {
    debug!("Processing DT project: {} ({})", dt_project.name, dt_project.uuid);

    let components = dt_client.get_project_components(&dt_project.uuid).await?;
    debug!("Found {} components in {}", components.len(), dt_project.name);

    let mut result = ProcessingResult {
        dt_project_name: dt_project.name.clone(),
        dt_project_uuid: dt_project.uuid.clone(),
        registered_names: Vec::new(),
        mapped_count: 0,
        unmapped_count: 0,
    };

    for component in components {
        // Filter components
        if !should_process_component(&component) {
            continue;
        }

        // Try to find repository URL
        match extract_repository_url_with_lookup(&component, db, registry_router).await {
            RepositoryLookupResult::Found(repo_url) => {
                match build_project_from_component(&component, &dt_project.name, repo_url) {
                    Ok(project) => {
                        // ✅ IMMEDIATE REGISTRATION (not batched)
                        match register_project_if_changed(db, foundation_id, &project).await {
                            Ok(registered) => {
                                if registered {
                                    debug!("Registered project: {}", project.name);
                                }
                                result.registered_names.push(project.name.clone());
                                result.mapped_count += 1;
                            }
                            Err(e) => {
                                error!("Failed to register {}: {}", project.name, e);
                                // ✅ CONTINUE to next component
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to build project for {}: {}", component.name, e);
                    }
                }
            }
            RepositoryLookupResult::NotFound(unmapped) => {
                // ✅ IMMEDIATE SAVE (not batched)
                if let Err(e) = db.save_unmapped_component(foundation_id, &unmapped).await {
                    error!("Failed to save unmapped {}: {}", unmapped.component_name, e);
                }
                result.unmapped_count += 1;
            }
        }
    }

    Ok(result)
}
```

### Step 4: Refactor Main Loop (45 min)
Replace lines 338-434 in `process_dt_foundation()`:

**REMOVE** (old batched code):
```rust
// Lines 338-388: Remove HashMap accumulation
let mut all_projects_to_register = HashMap::new();
let mut unmapped_components = Vec::new();

for dt_project in dt_projects {
    // ... lots of accumulation logic ...
    all_projects_to_register.insert(name, project); // REMOVE
}

// Lines 399-403: Remove batched unmapped save
for unmapped in unmapped_components {
    db.save_unmapped_component(...).await?;
}

// Lines 409-422: Remove batched registration
for (name, project) in &all_projects_to_register {
    db.register_project(...).await?;
}
```

**ADD** (new incremental code):
```rust
// Track what we've registered in this run (for cleanup phase)
let mut registered_in_this_run = HashSet::new();
let mut stats = ProcessingStats::default();

// ✅ PROCESS EACH DT PROJECT INDEPENDENTLY
for dt_project in dt_projects {
    match process_single_dt_project(
        &db,
        &dt_client,
        &dt_project,
        &foundation.foundation_id,
        registry_router.as_ref(),
    )
    .await
    {
        Ok(result) => {
            registered_in_this_run.extend(result.registered_names);
            stats.merge(&result);
        }
        Err(e) => {
            error!(
                "Failed to process DT project {} ({}): {}",
                dt_project.name, dt_project.uuid, e
            );
            stats.errors.push(format!("{}: {}", dt_project.name, e));
            // ✅ CONTINUE to next project instead of failing entire run
            continue;
        }
    }
}

info!(
    "DT foundation {}: {} projects processed, {} components mapped, {} unmapped, {} errors",
    foundation.foundation_id,
    stats.projects_processed,
    stats.components_mapped,
    stats.components_unmapped,
    stats.errors.len()
);
```

### Step 5: Update Cleanup (Keep mostly the same)
```rust
// Cleanup projects no longer in DT (lines 424-434)
if !registered_in_this_run.is_empty() {  // Changed from all_projects_to_register
    for name in projects_registered.keys() {
        if !registered_in_this_run.contains(name) {  // Changed from all_projects_to_register
            debug!(project = name, "unregistering");
            if let Err(err) = db.unregister_project(foundation_id, name).await {
                error!(?err, project = name, "error unregistering");
            }
        }
    }
}
```

### Step 6: Add Tests & Verify (1.5 hours)
See `RESILIENCE_TEST_REPORT.md` for complete test code.

```bash
# Compile
cargo build
# Should compile without errors

# Test
cargo test
# Expected: 85 passed (77 existing + 8 new)
# Expected: 0 failed
```

---

## Verification Checklist

After implementation, verify:

- [ ] `cargo build` - No errors, no warnings
- [ ] `cargo test` - All 85 tests pass
- [ ] Existing 77 tests still pass (no regressions)
- [ ] New 8 resilience tests pass
- [ ] Manual test: Process projects with one failure → others succeed
- [ ] Manual test: Restart after partial run → skip already-registered
- [ ] Manual test: Database error mid-run → continue processing

---

## Expected Test Results

### Before Implementation (Current)
```bash
cargo test
# 77 passed ✅
# 8 resilience tests NOT ADDED YET
```

### After Implementation (Target)
```bash
cargo test
# 85 passed ✅ (77 + 8 new)
# 0 failed ✅
```

---

## File Locations

**Target File**: `/Users/duderoot/git/research/foss/clomonitor/clomonitor-registrar/src/registrar.rs`
**Lines to Change**: 287-438 (process_dt_foundation function)

**Test Documentation**: `/Users/duderoot/git/research/foss/clomonitor/clomonitor-registrar/RESILIENCE_TEST_REPORT.md`

---

## Key Concepts

### Incremental Registration
**Before**: Accumulate all → register all
**After**: Register immediately after each project

### Failure Isolation
**Before**: First error stops everything
**After**: Log error, continue to next project

### Partial Success
**Before**: 999/1000 success → fail on #1000 → lose all 999
**After**: 999/1000 success → fail on #1000 → keep 999 ✅

### Restart Idempotency
**Before**: Restart processes all projects again
**After**: Restart skips already-registered (digest check)

---

## Common Pitfalls to Avoid

### Pitfall 1: Forget to call register_project_if_changed
```rust
// ❌ WRONG
let project = build_project_from_component(...)?;
result.registered_names.push(project.name); // Not registered!

// ✅ CORRECT
let project = build_project_from_component(...)?;
if register_project_if_changed(db, foundation_id, &project).await? {
    result.registered_names.push(project.name);
}
```

### Pitfall 2: Forget to continue on error
```rust
// ❌ WRONG
for dt_project in dt_projects {
    let result = process_single_dt_project(...).await?; // Stops on error
}

// ✅ CORRECT
for dt_project in dt_projects {
    match process_single_dt_project(...).await {
        Ok(result) => { /* handle */ },
        Err(e) => {
            error!("Error: {}", e);
            continue; // Keep going!
        }
    }
}
```

### Pitfall 3: Forget to save unmapped immediately
```rust
// ❌ WRONG
let mut unmapped_components = Vec::new();
unmapped_components.push(unmapped); // Batched!

// ✅ CORRECT
db.save_unmapped_component(foundation_id, &unmapped).await?; // Immediate!
```

---

## Testing Strategy

### Unit Tests (cargo test)
- 77 existing tests (verify no regressions)
- 8 new resilience tests (verify Phase 1 works)

### Integration Tests (manual)
1. **Resilience Test**: Start registrar, kill after 50%, restart → verify continues from 51%
2. **Failure Test**: Inject network error on project #2 → verify projects 1 and 3 succeed
3. **Database Test**: Simulate DB connection error → verify other projects continue

### Performance Test
- Before: Time to process 1000 projects
- After: Time to process 1000 projects
- Expected: ~5-10% slower (acceptable trade-off for resilience)

---

## Success Criteria

Phase 1 is complete when:
1. ✅ All 85 tests pass
2. ✅ No cargo warnings
3. ✅ Manual resilience test passes
4. ✅ Documentation updated
5. ✅ Code review approved

---

## Timeline

| Phase | Duration | Status |
|-------|----------|--------|
| TDD Red (Test Design) | 2 hours | ✅ DONE |
| Implementation | 3 hours | ⏳ NEXT |
| Testing & Debugging | 1 hour | ⏳ PENDING |
| Documentation | 30 min | ⏳ PENDING |
| **Total** | **6.5 hours** | **25% Complete** |

---

## Next Steps

1. **Read** `RESILIENCE_TEST_REPORT.md` for detailed test specs
2. **Implement** changes in `src/registrar.rs` (3 hours)
3. **Add** test code from report to test module (30 min)
4. **Run** `cargo test` and verify all pass
5. **Test** manually with simulated failures
6. **Deploy** to test environment
7. **Monitor** with real DT instance

---

## Questions?

- **Problem details**: See `docs/RESILIENCE_SUMMARY.md`
- **Implementation guide**: See `docs/IMPLEMENTATION_GUIDE_RESILIENCE.md`
- **Test specs**: See `RESILIENCE_TEST_REPORT.md`
- **Quick reference**: This file

---

**Status**: TDD Red Phase Complete ✅
**Ready**: For implementation (TDD Green) 🚀
**Time Estimate**: 3 hours of focused coding
**Confidence**: High (comprehensive test coverage)
