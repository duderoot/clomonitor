# Implementation Guide: Resilient DT Processing

## Executive Summary

The current DT processing implementation suffers from an "all-or-nothing" problem where failure at any point requires restarting from the beginning, wasting all previous work. This guide provides a phased approach to implement resilience.

**Quick Stats:**
- **Problem**: 999/1000 projects processed → fail on #1000 → lose ALL 999 projects
- **Solution**: Incremental registration → fail on #1000 → keep 999 projects → retry only #1000
- **Impact**: 99.9% of work preserved vs 0%

## Phase 1: Immediate Fix (Incremental Registration)

**Complexity**: Low
**Time**: 2-4 hours
**Impact**: Solves 80% of the problem
**Database Changes**: None required

### Changes Required

Modify `src/registrar.rs::process_dt_foundation()` function (lines 287-438).

#### Current Code Structure
```rust
async fn process_dt_foundation(...) -> Result<()> {
    // 1. Fetch ALL projects
    let dt_projects = dt_client.get_projects().await?;

    // 2. Accumulate ALL in memory
    let mut all_projects_to_register = HashMap::new();
    let mut unmapped_components = Vec::new();

    for dt_project in dt_projects {
        let components = dt_client.get_project_components(&dt_project.uuid).await?;

        for component in components {
            match extract_repository_url_with_lookup(...).await {
                RepositoryLookupResult::Found(repo_url) => {
                    let project = build_project_from_component(...)?;
                    all_projects_to_register.insert(project.name.clone(), project); // ⚠️ In memory only
                }
                RepositoryLookupResult::NotFound(unmapped) => {
                    unmapped_components.push(unmapped); // ⚠️ In memory only
                }
            }
        }
    }

    // 3. Batch save unmapped (LATE)
    for unmapped in unmapped_components {
        db.save_unmapped_component(...).await?;
    }

    // 4. Batch register projects (LATE)
    for (name, project) in &all_projects_to_register {
        db.register_project(foundation_id, project).await?;
    }

    // 5. Cleanup
    cleanup_removed_projects(...).await?;
}
```

#### Refactored Code (Incremental)

```rust
async fn process_dt_foundation(
    db: DynDB,
    foundation: Foundation,
    registry_api_cfg: RegistryApiConfig,
) -> Result<()> {
    let start = Instant::now();
    debug!("started (Dependency-Track)");

    let dt_config = match &foundation.data_source {
        DataSource::DependencyTrack(config) => config,
        _ => return Err(format_err!("Expected DependencyTrack data source")),
    };

    let registry_router = create_registry_router_if_enabled(&registry_api_cfg);
    let dt_client = DtHttpClient::new(dt_config.dt_url.clone(), dt_config.dt_api_key.clone());

    // Fetch all DT projects
    let dt_projects = dt_client.get_projects().await?;
    debug!("Found {} DT projects to process", dt_projects.len());

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

    // Cleanup projects no longer in DT
    cleanup_removed_projects(&db, &foundation.foundation_id, &registered_in_this_run).await?;

    debug!(duration_secs = start.elapsed().as_secs(), "completed");
    Ok(())
}

/// Process a single DT project and immediately register its components
async fn process_single_dt_project(
    db: &DynDB,
    dt_client: &DtHttpClient,
    dt_project: &DtProject,
    foundation_id: &str,
    registry_router: Option<&Arc<RegistryRouter>>,
) -> Result<ProcessingResult> {
    debug!(
        "Processing DT project: {} ({})",
        dt_project.name, dt_project.uuid
    );

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
            debug!(
                "Skipping component {} with classifier {}",
                component.name, component.classifier
            );
            continue;
        }

        // Try to find repository URL
        match extract_repository_url_with_lookup(&component, db, registry_router).await {
            RepositoryLookupResult::Found(repo_url) => {
                // Build CLOMonitor project
                match build_project_from_component(&component, &dt_project.name, repo_url) {
                    Ok(project) => {
                        // ✅ IMMEDIATE REGISTRATION (not batched)
                        match register_project_if_changed(db, foundation_id, &project).await {
                            Ok(registered) => {
                                if registered {
                                    debug!("Registered project: {}", project.name);
                                    result.registered_names.push(project.name.clone());
                                    result.mapped_count += 1;
                                } else {
                                    debug!("Skipped registration (unchanged): {}", project.name);
                                    result.registered_names.push(project.name.clone());
                                    result.mapped_count += 1;
                                }
                            }
                            Err(e) => {
                                error!("Failed to register {}: {}", project.name, e);
                                // ✅ CONTINUE to next component
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to build project for component {}: {}", component.name, e);
                    }
                }
            }
            RepositoryLookupResult::NotFound(unmapped) => {
                // ✅ IMMEDIATE SAVE (not batched)
                if let Err(e) = db.save_unmapped_component(foundation_id, &unmapped).await {
                    error!("Failed to save unmapped component {}: {}", unmapped.component_name, e);
                }
                result.unmapped_count += 1;
            }
        }
    }

    debug!(
        "Completed DT project {}: {} mapped, {} unmapped",
        dt_project.name, result.mapped_count, result.unmapped_count
    );

    Ok(result)
}

/// Register a project only if it doesn't exist or has changed
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

/// Cleanup projects that are no longer in DT
async fn cleanup_removed_projects(
    db: &DynDB,
    foundation_id: &str,
    registered_in_this_run: &HashSet<String>,
) -> Result<()> {
    if registered_in_this_run.is_empty() {
        debug!("No projects registered, skipping cleanup");
        return Ok(());
    }

    let projects_registered = db.foundation_projects(foundation_id).await?;

    for name in projects_registered.keys() {
        if !registered_in_this_run.contains(name) {
            debug!("Unregistering removed project: {}", name);
            if let Err(err) = db.unregister_project(foundation_id, name).await {
                error!(?err, project = name, "error unregistering");
            }
        }
    }

    Ok(())
}

// Helper structs
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

struct ProcessingResult {
    dt_project_name: String,
    dt_project_uuid: String,
    registered_names: Vec<String>,
    mapped_count: usize,
    unmapped_count: usize,
}
```

### Key Changes Explained

1. **Extract `process_single_dt_project` function**
   - Processes ONE DT project at a time
   - Returns result or error (doesn't crash entire run)
   - Failure isolated to single project

2. **Immediate registration** (line ~60-70 in new code)
   - `register_project_if_changed()` called IMMEDIATELY after mapping
   - Not accumulated in HashMap
   - Project committed to DB before moving to next component

3. **Immediate unmapped save** (line ~80-85 in new code)
   - `save_unmapped_component()` called IMMEDIATELY
   - Not accumulated in Vec
   - Tracked in DB right away

4. **Continue on error** (line ~40-50 in new code)
   - `match process_single_dt_project()` with continue on Err
   - One failed project doesn't stop others
   - Errors logged and tracked in stats

5. **Digest-based deduplication** (`register_project_if_changed`)
   - Prevents re-registering unchanged projects
   - Enables idempotent restarts
   - Database digest check before registration

### Testing the Changes

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_incremental_registration_resilience() {
        // Setup mocks
        let mut dt_client_mock = MockDtClient::new();
        let mut db_mock = MockDB::new();

        // Mock 3 DT projects
        dt_client_mock
            .expect_get_projects()
            .returning(|| Ok(vec![
                DtProject { uuid: "p1".into(), name: "Project1".into(), ... },
                DtProject { uuid: "p2".into(), name: "Project2".into(), ... },
                DtProject { uuid: "p3".into(), name: "Project3".into(), ... },
            ]));

        // Mock components for project 1 (success)
        dt_client_mock
            .expect_get_project_components()
            .with(eq("p1"))
            .returning(|_| Ok(vec![/* valid component */]));

        // Mock components for project 2 (FAILURE)
        dt_client_mock
            .expect_get_project_components()
            .with(eq("p2"))
            .returning(|_| Err(anyhow!("Network timeout")));

        // Mock components for project 3 (success)
        dt_client_mock
            .expect_get_project_components()
            .with(eq("p3"))
            .returning(|_| Ok(vec![/* valid component */]));

        // Expect registrations for projects 1 and 3 (NOT project 2)
        db_mock
            .expect_register_project()
            .times(2)  // ✅ Only 2, not 3
            .returning(|_, _| Box::pin(future::ready(Ok(()))));

        // Run process
        let result = process_dt_foundation(
            Arc::new(db_mock),
            foundation,
            registry_api_cfg,
        ).await;

        // ✅ Should succeed despite project 2 failure
        assert!(result.is_ok());
        // Projects 1 and 3 were registered
        // Project 2 was skipped due to error
    }

    #[tokio::test]
    async fn test_restart_skips_registered_projects() {
        let mut db_mock = MockDB::new();

        // First run: project1 gets registered
        // Second run (restart): project1 should be skipped

        db_mock
            .expect_foundation_projects()
            .returning(|_| {
                let mut map = HashMap::new();
                map.insert("project1".to_string(), Some("digest123".to_string()));
                Box::pin(future::ready(Ok(map)))
            });

        db_mock
            .expect_register_project()
            .times(0);  // ✅ Should NOT register (digest unchanged)

        let project = Project {
            name: "project1".into(),
            digest: Some("digest123".into()),  // Same digest
            ...
        };

        let registered = register_project_if_changed(&Arc::new(db_mock), "foundation", &project)
            .await
            .unwrap();

        assert!(!registered);  // ✅ Returns false (not registered)
    }
}
```

### Deployment Steps

1. **Review changes**
   ```bash
   git diff src/registrar.rs
   ```

2. **Run tests**
   ```bash
   cargo test process_dt_foundation
   cargo test incremental_registration
   ```

3. **Test with small DT instance**
   ```bash
   # Use test configuration with small DT instance
   RUST_LOG=debug cargo run -- --config test-config.yaml
   ```

4. **Monitor logs**
   Look for:
   - `"Registered project: ..."` - Incremental registrations
   - `"Failed to process DT project X"` - Isolated failures
   - `"Completed DT project X: N mapped, M unmapped"` - Progress

5. **Verify restart behavior**
   - Start process
   - Kill after processing 50% of projects
   - Restart
   - Should skip already-registered projects (via digest check)

### Expected Behavior

**Before (Current)**:
```
Processing 1000 DT projects...
[fetching all projects...]
[fetching all components...]
[mapping all components...]
ERROR: Failed to register project 999
❌ ALL WORK LOST
Restart: Process all 1000 projects again
```

**After (Incremental)**:
```
Processing DT project 1/1000... ✅ Registered
Processing DT project 2/1000... ✅ Registered
...
Processing DT project 500/1000... ❌ Failed (network timeout)
  Error logged, continuing...
Processing DT project 501/1000... ✅ Registered
...
Processing DT project 1000/1000... ✅ Registered

Summary: 999/1000 succeeded, 1 failed

Restart:
  Checking DT project 1... Digest unchanged, skipped
  Checking DT project 2... Digest unchanged, skipped
  ...
  Checking DT project 500... Not found, processing... ✅ Registered
  Checking DT project 501... Digest unchanged, skipped
  ...

Summary: 1/1000 processed (999 already registered)
```

## Phase 2: Checkpoint/Resume (Future Enhancement)

See `RESILIENCE_ANALYSIS.md` for full details.

**Requirements**:
- Database schema changes (new `dt_processing_state` table)
- Checkpoint manager module
- Resume logic

**Benefits over Phase 1**:
- Full visibility into progress
- Can retry only failed projects
- No duplicate work on restart (even failed components)

## Phase 3: Advanced Optimizations (Future)

1. **Streaming with backpressure**
2. **Concurrent DT project processing**
3. **Batch commits with configurable size**
4. **Circuit breaker for registry APIs**

## Comparison Table

| Scenario | Current | Phase 1 | Phase 2 | Phase 3 |
|----------|---------|---------|---------|---------|
| Process 1000 projects, fail on #500 | Lose all | Lose #500 only | Retry #500 only | Concurrent + retry |
| Process 1000 projects, restart | Redo all 1000 | Check digest, skip unchanged | Resume from checkpoint | Resume with streaming |
| Memory usage (50K components) | 50K objects | <100 objects | <100 objects | <500 objects |
| Registration latency | 60min then all | <1sec per project | <1sec per project | <1sec per batch |
| Recovery time (after crash) | 60min | ~1sec × failed projects | 0 (resume) | 0 (resume) |

## Recommendation

✅ **Implement Phase 1 immediately**
- Minimal code changes
- No schema changes
- Solves 80% of the problem
- Can be deployed today

Then plan for Phase 2 based on operational needs (checkpoint/resume for very large DT instances).

## Questions & Troubleshooting

### Q: Will this make processing slower?
**A**: Minimal impact (~5-10% slower due to individual DB calls), but acceptable given resilience gain. Connection pooling mitigates this.

### Q: What if the database itself fails?
**A**: Phase 1 helps (less work lost), but Phase 2 (checkpoint) is better for database failures.

### Q: How do I test this locally?
**A**: Use a small DT instance (5-10 projects) and intentionally cause failures (network timeout, invalid data) to verify resilience.

### Q: Will this work with existing database functions?
**A**: Yes! No changes to database schema or functions required for Phase 1.

## See Also

- `RESILIENCE_ANALYSIS.md` - Detailed analysis of all strategies
- `docs/diagrams/05b_sequence_dt_processing_resilient.puml` - Resilient sequence diagram
- `docs/diagrams/05c_sequence_dt_processing_checkpoint.puml` - Checkpoint strategy diagram
- `docs/diagrams/09_comparison_current_vs_resilient.puml` - Visual comparison
