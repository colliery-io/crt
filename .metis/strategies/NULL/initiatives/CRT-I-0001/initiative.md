---
id: css-to-shader-pipeline-research
level: initiative
title: "CSS to Shader Pipeline Research"
short_code: "CRT-I-0001"
created_at: 2025-11-25T01:27:10.194428+00:00
updated_at: 2025-11-25T01:30:18.998923+00:00
parent: CRT-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/decompose"


exit_criteria_met: false
estimated_complexity: M
strategy_id: NULL
initiative_id: css-to-shader-pipeline-research
---

# CSS to Shader Pipeline Research Initiative

Research initiative to explore and prototype the CSS-to-shader transformation pipeline for CRT terminal rendering effects.

## Context

CRT needs to apply visual effects (CRT phosphor glow, scanlines, screen curvature, etc.) to terminal content. A key architectural question is how to bridge the styling world (CSS-like declarations) with GPU shader programs that render these effects.

This initiative explores the feasibility and approaches for:
- Parsing/representing CSS-like style declarations
- Transforming those declarations into shader parameters or generated shader code
- Runtime application of styles to GPU rendering

## Goals & Non-Goals

**Goals:**
- Understand the problem space and constraints
- Prototype at least 2-3 different approaches
- Identify trade-offs between approaches (flexibility vs performance vs complexity)
- Produce an ADR documenting the chosen architecture

**Non-Goals:**
- Production-ready implementation
- Full CSS specification support
- Performance optimization (beyond basic feasibility)

## Research Questions

1. **Representation**: How should CSS-like styles be represented in Rust?
2. **Mapping**: How do CSS properties map to shader uniforms/parameters?
3. **Generation**: Should shaders be generated at compile-time, runtime, or use a hybrid approach?
4. **Composition**: How do multiple effects compose (layering, blending)?
5. **Hot-reload**: Is live style editing feasible for development?

## Approaches to Explore

### Approach A: Static Shader + Uniform Mapping
CSS properties map directly to shader uniforms. Single pre-written shader.

### Approach B: Shader Generation
Generate WGSL/GLSL from CSS declarations at compile or runtime.

### Approach C: Effect Graph
Build a composable effect node graph that CSS configures.

## Deliverables

- [ ] Working prototype demonstrating at least one approach
- [ ] Documentation of findings for each approach explored
- [ ] ADR: CSS-to-Shader Pipeline Architecture (CRT-A-XXXX)

## Notes & Findings

{Document learnings as research progresses}