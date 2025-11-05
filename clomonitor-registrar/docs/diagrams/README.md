# Architecture Diagrams

This directory contains PlantUML diagrams documenting the CLOMonitor Registrar architecture using the C4 model.

## Diagram Files

### C4 Model Diagrams

1. **01_system_context.puml** - Level 1: System Context
   - Shows the registrar's interactions with external systems
   - Actors: System Administrator
   - External Systems: YAML files, Dependency-Track, package registries, PostgreSQL

2. **02_container.puml** - Level 2: Container Diagram
   - Shows the main runtime containers
   - Containers: Registrar Application, PostgreSQL, In-Memory Cache

3. **03_component.puml** - Level 3: Component Diagram
   - Shows internal modules and their relationships
   - Components: main.rs, registrar.rs, db.rs, dt_client.rs, dt_mapper.rs, registry_apis/

### Runtime Views

4. **04_sequence_yaml_processing.puml** - YAML Foundation Processing
   - Shows the sequential flow for processing foundation YAML files
   - Demonstrates: config loading, YAML fetching, project registration/unregistration

5. **05_sequence_dt_processing.puml** - Dependency-Track Processing (Current Implementation)
   - Shows the sequential flow for processing DT instances
   - Demonstrates: project/component fetching, URL resolution, caching
   - ⚠️ **Note**: This shows the current "all-or-nothing" approach with resilience issues

5b. **05b_sequence_dt_processing_resilient.puml** - DT Processing with Incremental Registration
   - Shows the IMPROVED sequential flow with resilience
   - Demonstrates: incremental registration, per-project commits, failure isolation
   - ✅ **Recommended**: Phase 1 improvement (see `IMPLEMENTATION_GUIDE_RESILIENCE.md`)

5c. **05c_sequence_dt_processing_checkpoint.puml** - DT Processing with Checkpoint/Resume
   - Shows ADVANCED resilience with checkpoint/resume capability
   - Demonstrates: state tracking, resume from failure, retry failed projects
   - 🔮 **Future**: Phase 2 improvement with database checkpointing

6. **06_activity_url_resolution.puml** - URL Resolution Strategy
   - Shows the decision flow for resolving component repository URLs
   - Demonstrates: externalReferences, purl parsing, cache lookup, registry API calls

### Supporting Diagrams

7. **07_deployment.puml** - Deployment View
   - Shows runtime deployment topology
   - Execution modes: cron, manual, Docker

8. **08_data_model.puml** - Data Model
   - Shows database entities and relationships
   - Tables: foundation, project, repository, component_mapping, unmapped_component

9. **09_comparison_current_vs_resilient.puml** - Current vs Resilient Comparison
   - Visual comparison of "all-or-nothing" vs incremental approaches
   - Highlights failure scenarios and recovery behavior
   - Useful for understanding the resilience improvements

## How to Render Diagrams

### Using PlantUML CLI

```bash
# Install PlantUML
brew install plantuml  # macOS
# or
apt-get install plantuml  # Ubuntu/Debian

# Render a single diagram
plantuml 01_system_context.puml

# Render all diagrams
plantuml *.puml

# Render to specific format
plantuml -tpng 01_system_context.puml  # PNG
plantuml -tsvg 01_system_context.puml  # SVG
```

### Using VS Code

1. Install the **PlantUML** extension
2. Open any `.puml` file
3. Press `Alt+D` (or `Option+D` on macOS) to preview
4. Right-click → "Export Current Diagram" to save as PNG/SVG

### Using Online Editor

1. Visit https://www.plantuml.com/plantuml/uml/
2. Copy and paste the diagram content
3. View rendered diagram in browser
4. Download as PNG/SVG

### Using AsciiDoc

The diagrams are embedded in the main `architecture.adoc` file. To render the full documentation:

```bash
# Install AsciiDoctor with PlantUML support
gem install asciidoctor asciidoctor-diagram

# Render to HTML
asciidoctor -r asciidoctor-diagram ../architecture.adoc

# Render to PDF (requires asciidoctor-pdf)
gem install asciidoctor-pdf
asciidoctor-pdf -r asciidoctor-diagram ../architecture.adoc
```

## Diagram Conventions

### C4 Model Colors

- **Blue**: Internal systems/containers
- **Gray**: External systems
- **Green**: Databases
- **Orange**: People/actors

### Notation

- **Solid lines**: Direct dependencies/calls
- **Dashed lines**: Asynchronous/indirect relationships
- **Arrows**: Direction of data flow

## Resilience Documentation

**Important**: The DT processing implementation has known resilience issues (all-or-nothing approach).

See these documents for details and solutions:
- `../RESILIENCE_ANALYSIS.md` - Comprehensive analysis of the problem and 5 solution strategies
- `../IMPLEMENTATION_GUIDE_RESILIENCE.md` - Step-by-step guide to implement incremental registration (Phase 1)
- Diagrams 05, 05b, 05c, 09 - Visual comparison of approaches

## Maintenance

When updating these diagrams:

1. Update the standalone `.puml` file in this directory
2. Update the corresponding diagram in `../architecture.adoc`
3. Regenerate rendered outputs (PNG/SVG) if needed
4. Update this README if adding new diagrams

## C4 Model Resources

- [C4 Model Homepage](https://c4model.com/)
- [C4-PlantUML GitHub](https://github.com/plantuml-stdlib/C4-PlantUML)
- [PlantUML Documentation](https://plantuml.com/)

## Dependencies

The diagrams use the C4-PlantUML library which is automatically fetched from GitHub when rendering:

```plantuml
!include https://raw.githubusercontent.com/plantuml-stdlib/C4-PlantUML/master/C4_Context.puml
```

No local installation of C4-PlantUML is required.
