---
id: installation-and-first-run-setup
level: initiative
title: "Installation and First-Run Setup"
short_code: "CRT-I-0012"
created_at: 2025-11-27T10:06:55.230755+00:00
updated_at: 2025-11-29T17:19:41.684901+00:00
parent: 
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: L
strategy_id: NULL
initiative_id: installation-and-first-run-setup
---

# Installation and First-Run Setup Initiative

*This template includes sections for various types of initiatives. Delete sections that don't apply to your specific use case.*

## Context

Create installer packages for macOS and Linux that set up the CRT terminal with all required assets. Goal is to keep the binary slim by not bundling assets - instead, installers copy provided themes and fonts to `~/.config/crt/`.

### Target Structure
```
~/.config/crt/
  config.toml           # User config (created if missing)
  themes/               # Provided + user custom themes
    synthwave.css
    dracula.css
    solarized.css
    minimal.css
  fonts/                # Provided Nerd fonts
    MesloLGS NF Regular.ttf
    MesloLGS NF Bold.ttf
    MesloLGS NF Italic.ttf
    MesloLGS NF Bold Italic.ttf
```

### Installer Formats
- **macOS**: PKG installer (runs scripts to set up config dir)
- **Linux**: Shell script installer

### Binary Locations
- **macOS**: `/Applications/CRT.app/Contents/MacOS/crt`
- **Linux**: `/usr/local/bin/crt`

## Goals & Non-Goals

**Goals:**
- Keep binary slim - no bundled assets
- Single config location (`~/.config/crt/`) for simplicity
- PKG installer for macOS with proper setup
- Shell script installer for Linux
- Include provided Nerd fonts and themes

**Non-Goals:**
- Windows support (future initiative)
- Auto-update mechanism
- App store distribution

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