---
id: gpu-resource-pooling-and-sharing
level: initiative
title: "GPU Resource Pooling and Sharing"
short_code: "CRT-I-0019"
created_at: 2025-11-29T17:19:07.836909+00:00
updated_at: 2025-11-29T17:19:07.836909+00:00
parent: CRT-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/discovery"


exit_criteria_met: false
estimated_complexity: L
strategy_id: NULL
initiative_id: gpu-resource-pooling-and-sharing
---

# GPU Resource Pooling and Sharing

## Context

Each window currently creates its own GPU resources: pipelines, bind group layouts, samplers, instance buffers, and render target textures. This results in ~40MB of GPU memory per additional window, most of which is redundant since many resources are stateless and identical across windows.

The Vello::Renderer is already shared via `Arc<Mutex<Option<Renderer>>>` in SharedGpuState, saving ~187MB. This initiative extends that pattern to all shareable GPU resources.

## Goals

- Share all stateless GPU resources (pipelines, bind group layouts, samplers) globally
- Pool instance buffers for reuse across window lifecycle (~5.5MB per window)
- Pool render target textures by size bucket (~32MB per window)
- Share fixed-size tab glyph cache (~1MB per window)
- Reduce per-window GPU memory from ~40MB to ~2MB

## Non-Goals

- CPU-side memory optimizations (covered by CRT-I-0017, CRT-I-0018)
- Per-frame allocation optimizations
- Glyph cache sharing for terminal text (varies by zoom level)

## Architecture

### New Module Structure
```
src/gpu/
  mod.rs              - Re-exports, updated SharedGpuState
  shared_pipelines.rs - All render pipelines, bind group layouts, samplers
  buffer_pool.rs      - Instance/uniform buffer pooling with RAII
  texture_pool.rs     - Render target texture pooling by size bucket
```

### SharedPipelines
Consolidate all stateless GPU resources:
- GridRenderer pipeline + bind group layout
- RectRenderer pipeline + bind group layout
- BackgroundPipeline, CompositePipeline, CrtPipeline, BackgroundImagePipeline
- Effects blit pipeline
- Shared linear/nearest samplers (consolidate ~8 identical samplers to 2)

### Buffer Pool
RAII-based pool for instance and uniform buffers:
- GridInstance class: 1.5MB (32K * 48 bytes)
- RectInstance class: 512KB (16K * 32 bytes)
- SmallUniform class: 256 bytes
- Checkout/return semantics with automatic return on Drop

### Texture Pool
Pool render target textures by power-of-two size buckets:
- text_texture, crt_texture
- TerminalVelloRenderer, EffectsRenderer, VelloTabBarRenderer targets
- Automatic resize handling (return old, checkout new)

### Shared Tab Glyph Cache
Move fixed 12px tab glyph cache to SharedGpuState since it never scales with zoom.

## Memory Impact

| Scenario | Current | After Pooling | Savings |
|----------|---------|---------------|---------|
| 1 window | ~40 MB | ~40 MB | 0% |
| 2 windows | ~80 MB | ~45 MB | 44% |
| 5 windows | ~200 MB | ~55 MB | 73% |

## Critical Files

- `src/gpu.rs` - Split into module, add pools to SharedGpuState
- `src/main.rs` - Update create_window to use shared resources
- `crates/crt-renderer/src/lib.rs` - Refactor pipeline structs
- `crates/crt-renderer/src/grid_renderer.rs` - Accept shared pipeline + pooled buffer
- `crates/crt-renderer/src/rect_renderer.rs` - Same pattern
- `crates/crt-renderer/src/effects/renderer.rs` - Use shared blit pipeline
- `crates/crt-renderer/src/tab_bar/mod.rs` - Use shared tab glyph cache Initiative

*This template includes sections for various types of initiatives. Delete sections that don't apply to your specific use case.*

## Context **[REQUIRED]**

{Describe the context and background for this initiative}

## Goals & Non-Goals **[REQUIRED]**

**Goals:**
- {Primary objective 1}
- {Primary objective 2}

**Non-Goals:**
- {What this initiative will not address}

## Requirements **[CONDITIONAL: Requirements-Heavy Initiative]**

{Delete if not a requirements-focused initiative}

### User Requirements
- **User Characteristics**: {Technical background, experience level, etc.}
- **System Functionality**: {What users expect the system to do}
- **User Interfaces**: {How users will interact with the system}

### System Requirements
- **Functional Requirements**: {What the system should do - use unique identifiers}
  - REQ-001: {Functional requirement 1}
  - REQ-002: {Functional requirement 2}
- **Non-Functional Requirements**: {How the system should behave}
  - NFR-001: {Performance requirement}
  - NFR-002: {Security requirement}

## Use Cases **[CONDITIONAL: User-Facing Initiative]**

{Delete if not user-facing}

### Use Case 1: {Use Case Name}
- **Actor**: {Who performs this action}
- **Scenario**: {Step-by-step interaction}
- **Expected Outcome**: {What should happen}

### Use Case 2: {Use Case Name}
- **Actor**: {Who performs this action}
- **Scenario**: {Step-by-step interaction}
- **Expected Outcome**: {What should happen}

## Architecture **[CONDITIONAL: Technically Complex Initiative]**

{Delete if not technically complex}

### Overview
{High-level architectural approach}

### Component Diagrams
{Describe or link to component diagrams}

### Class Diagrams
{Describe or link to class diagrams - for OOP systems}

### Sequence Diagrams
{Describe or link to sequence diagrams - for interaction flows}

### Deployment Diagrams
{Describe or link to deployment diagrams - for infrastructure}

## Detailed Design **[REQUIRED]**

{Technical approach and implementation details}

## UI/UX Design **[CONDITIONAL: Frontend Initiative]**

{Delete if no UI components}

### User Interface Mockups
{Describe or link to UI mockups}

### User Flows
{Describe key user interaction flows}

### Design System Integration
{How this fits with existing design patterns}

## Testing Strategy **[CONDITIONAL: Separate Testing Initiative]**

{Delete if covered by separate testing initiative}

### Unit Testing
- **Strategy**: {Approach to unit testing}
- **Coverage Target**: {Expected coverage percentage}
- **Tools**: {Testing frameworks and tools}

### Integration Testing
- **Strategy**: {Approach to integration testing}
- **Test Environment**: {Where integration tests run}
- **Data Management**: {Test data strategy}

### System Testing
- **Strategy**: {End-to-end testing approach}
- **User Acceptance**: {How UAT will be conducted}
- **Performance Testing**: {Load and stress testing}

### Test Selection
{Criteria for determining what to test}

### Bug Tracking
{How defects will be managed and prioritized}

## Alternatives Considered **[REQUIRED]**

{Alternative approaches and why they were rejected}

## Implementation Plan **[REQUIRED]**

{Phases and timeline for execution}