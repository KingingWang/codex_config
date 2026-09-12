# Design

## Source of truth

- Status: Active
- Last refreshed: 2026-09-12
- Primary product surfaces: Chinese-first Rust / egui desktop configuration assistant.
- Evidence reviewed: README.md, src/app.rs, src/page.rs, src/ui/, src/dialogs.rs, tests/gui.rs, assets/icon/.
- Observed: existing UI exposes technical fields too early and uses crowded fixed-width editors.
- Assumption: user grants visual direction; retain the native application and every configuration capability.

## Brand

- Personality: calm, approachable, precise.
- Trust signals: explicit saved state, local editing, automatic backups, actionable validation.
- Avoid: terminal-like density, purple gradients, unverified connection claims.

## Product goals

- Goals: understand the next action, choose a model, choose safe permissions, save without learning TOML.
- Non-goals: new backend, dependencies, authentication changes or silently applied presets.
- Success signals: a usable first screen; advanced options folded away; all operations reachable and tested.

## Personas and jobs

- Primary personas: Chinese-speaking beginners and experienced users.
- User jobs: connect a model source, choose a model, set permissions, save safely.
- Key contexts: first launch, routine model switching, repairing invalid configuration.

## Information architecture

- Primary navigation: 开始使用, 模型管理, 服务商.
- Core routes/screens: separate secondary group 配置档, 高级选项, 源文件编辑; retain six Page variants.
- Content hierarchy: welcome and next action → model / permissions → optional preferences / checks / files.
- Global actions: prominent save, secondary preview/undo, explicitly separate restart.

## Design principles

- One obvious next action; progressively disclose rare settings.
- Chinese task labels first, raw field keys in tooltips.
- Never change settings merely by visiting a page.
- Tradeoffs: readable forms over displaying every field at once; preserve expert tools.

## Visual language

- Color: warm paper #F5F6F2, white surfaces, ink #243A35, botanical green #26715B, sage selections, semantic amber/red.
- Typography: CJK-capable native fonts; 26–30 pt headings, 14–15 pt body, 12–13 pt metadata; monospace for code only.
- Spacing/layout rhythm: 8 pt rhythm, 20–24 pt card padding, 16–24 pt gaps.
- Shape/radius/elevation: 12–16 pt rounded cards, fine borders, restrained shadows.
- Motion: native hover/focus feedback, no perpetual ornamental animation.
- Imagery/iconography: existing Phosphor glyphs and native geometric motifs; no remote assets.

## Components

- Existing components to reuse: cards, fields, dropdowns, notes, buttons, list rows, modal, empty states.
- New/changed components: welcome panel, guided next action, permission presets, grouped sidebar, save/status bar.
- Variants and states: primary/secondary/danger actions, labelled statuses, visible keyboard focus.
- Token/component ownership: src/ui/theme.rs and src/ui/widgets.rs; no parallel design system.

## Accessibility

- Target standard: practical desktop accessibility with AA text-contrast goals, not a certification claim.
- Keyboard/focus behavior: native egui controls; platform-aware save shortcut.
- Contrast/readability: dark text on light surfaces; labels alongside semantic color.
- Screen-reader semantics: labelled native buttons and inputs.
- Reduced motion and sensory considerations: no flashing or continuous animation.

## Responsive behavior

- Supported breakpoints/devices: desktop minimum 1000×660, normal 1360×880, large 1440×940.
- Layout adaptations: narrow fields stack; flexible split views; scroll every long form.
- Touch/hover differences: 32–36 pt targets where practical; essential guidance not tooltip-only.

## Interaction states

- Loading: visible progress, disable duplicate network requests.
- Empty: distinguish built-in defaults from missing custom files; offer relevant next steps.
- Error: explanatory text and repair destination; invalid saves blocked.
- Success: saved means written, not connected; restart reminder.
- Disabled: saving unavailable without changes.
- Offline/slow network: local editing remains usable.

## Content voice

- Tone: friendly, concise, non-patronizing.
- Terminology: 模型 = the AI; 服务商 = where requests go; 配置档 = saved settings.
- Microcopy rules: verbs for actions, no implied connection test, sandbox and approval are separate.

## Implementation constraints

- Framework/styling system: existing egui 0.36 / eframe native Rust.
- Design-token constraints: shared semantic palette; extend existing components.
- Performance constraints: no remote fonts, new dependencies or background services.
- Compatibility constraints: retain unknown fields/comments, explicit save, backup, protected restart.
- Test/screenshot expectations: baseline tests, navigation, presets, disclosure, save/discard/raw safeguards, empty/error states, small-window rendering, real native launch.

## Implementation and verification plan

1. Run existing fixture-based baseline tests.
2. Redesign shared tokens/widgets, home, shell and editor presentation without changing storage behavior.
3. Test new interactions and safety boundaries in disposable directories; never call live providers or modify personal configuration in tests.
4. Run formatting, compilation, Clippy and tests; inspect actual screenshots and smoke-run the binary.

## Open questions

- [ ] Optional dark mode can be a future preference; this design prioritizes one coherent light theme.
- [ ] Windows/macOS visual review requires those environments; current verification uses Linux.
