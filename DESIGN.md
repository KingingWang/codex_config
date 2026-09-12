# Design

## Source of truth

- Status: Active
- Last refreshed: 2026-09-12
- Primary product surfaces: Chinese-first Rust / egui desktop configuration assistant.
- Evidence reviewed: README.md, src/app.rs, src/page.rs, src/ui/, src/dialogs.rs, tests/gui.rs, assets/icon/.
- Observed: the user explicitly rejects the green light theme and requests remote SSH targets with an SSH-config alias dropdown.
- Assumption: retain the native application; remote targets run Linux/macOS with Python 3 and use existing SSH key/agent authentication.

## Brand

- Personality: calm, approachable, precise.
- Trust signals: explicit target and saved state, automatic backups, actionable validation, no automatic SSH connections.
- Avoid: terminal-like density, purple gradients, unverified connection claims.

## Product goals

- Goals: choose a local or SSH environment, edit model/provider/permission settings, save safely to the displayed target.
- Non-goals: new dependencies, password/key storage, SSH-config editing, remote process management or silently applied presets.
- Success signals: a usable first screen; advanced options folded away; all operations reachable and tested.

## Personas and jobs

- Primary personas: Chinese-speaking beginners and experienced users.
- User jobs: connect a model source, choose a model, set permissions, save safely.
- Key contexts: local development, remote development machines, routine model switching, repairing invalid configuration.

## Information architecture

- Primary navigation: 开始使用, 模型管理, 服务商.
- Core routes/screens: separate secondary group 配置档, 高级选项, 源文件编辑; retain six Page variants.
- Content hierarchy: current environment → model / permissions → optional preferences / checks / files.
- Global actions: environment switch, prominent save, secondary preview/undo, local-only restart.
- Environment switcher: always-reachable sidebar action; local folder or SSH alias dropdown, optional remote CODEX_HOME, explicit connect/load, progress and actionable failures.

## Design principles

- One obvious next action; progressively disclose rare settings.
- Chinese task labels first, raw field keys in tooltips.
- Never change settings merely by visiting a page.
- Tradeoffs: readable forms over displaying every field at once; preserve expert tools.

## Visual language

- Color: charcoal canvas #111318, sidebar #15181E, raised surfaces #1C2028, inputs #12151B, borders #303744; off-white text #E7EBF2 and muted #9BA6B8. Restrained blue #719DF4 for emphasis; green only for semantic success.
- Typography: CJK-capable native fonts; 26–30 pt headings, 14–15 pt body, 12–13 pt metadata; monospace for code only.
- Spacing/layout rhythm: 8 pt rhythm, 20–24 pt card padding, 16–24 pt gaps.
- Shape/radius/elevation: 12–16 pt rounded cards, fine borders, restrained shadows.
- Motion: native hover/focus feedback, no perpetual ornamental animation.
- Imagery/iconography: existing Phosphor glyphs and native geometric motifs; no remote assets.

## Components

- Existing components to reuse: cards, fields, dropdowns, notes, buttons, list rows, modal, empty states.
- New/changed components: compact workspace summary, environment switcher, SSH alias dropdown and progress/error states, grouped sidebar, target-aware save/status bar.
- Variants and states: primary/secondary/danger actions, labelled statuses, visible keyboard focus.
- Token/component ownership: src/ui/theme.rs and src/ui/widgets.rs; no parallel design system.

## Accessibility

- Target standard: practical desktop accessibility with AA text-contrast goals, not a certification claim.
- Keyboard/focus behavior: native egui controls; platform-aware save shortcut.
- Contrast/readability: light text on dark surfaces; labels alongside semantic color; no pure-white panels.
- Screen-reader semantics: labelled native buttons and inputs.
- Reduced motion and sensory considerations: no flashing or continuous animation.

## Responsive behavior

- Supported breakpoints/devices: desktop minimum 1000×660, normal 1360×880, large 1440×940.
- Layout adaptations: narrow fields stack; flexible split views; scroll every long form.
- Touch/hover differences: 32–36 pt targets where practical; essential guidance not tooltip-only.

## Interaction states

- Loading: visible progress, disable duplicate requests and edits/switches during SSH operations; load can be cancelled, saving must finish or report uncertainty.
- Empty: distinguish built-in defaults from missing custom files; offer relevant next steps.
- Error: explanatory text and repair destination; invalid saves blocked.
- Success: saved means written to the named target, not continuously connected; remote restart is manual.
- Disabled: saving unavailable without changes.
- Offline/slow network: no automatic reconnect; bounded SSH timeout; failed load keeps the previous document and failed save keeps edits.

## Content voice

- Tone: friendly, concise, non-patronizing.
- Terminology: 模型 = the AI; 服务商 = where requests go; 配置档 = saved settings.
- Microcopy rules: verbs for actions, no implied connection test, sandbox and approval are separate.

## Implementation constraints

- Framework/styling system: existing egui 0.36 / eframe native Rust.
- Design-token constraints: shared semantic palette; extend existing components.
- Performance constraints: no remote fonts, new dependencies or background services.
- Compatibility constraints: retain unknown fields/comments, explicit save, backup, protected local restart. Remote files never pass through local filesystem APIs; no local provider probe or restart in SSH mode.
- SSH constraints: use system OpenSSH and existing config; list literal Host aliases including Include files without executing SSH configuration; preserve host-key checking, no passwords/private keys read or stored. Remote helper uses only Python standard library and stdin JSON.
- Test/screenshot expectations: baseline tests, navigation, presets, disclosure, save/discard/raw safeguards, empty/error states, small-window rendering, real native launch.

## Implementation and verification plan

1. Run existing fixture-based baseline tests.
2. Update shared dark tokens, home and shell; add environment-aware remote snapshot loading/saving without weakening local persistence.
3. Test SSH alias parsing, remote helper read/save/conflict/backup boundaries and GUI target switching in disposable directories; never connect to real hosts or modify personal configuration in tests.
4. Run formatting, compilation, Clippy and tests; inspect actual screenshots and smoke-run the binary.

## Open questions

- [ ] Password-only SSH and remote Windows hosts are not supported in this iteration.
- [ ] Windows/macOS visual review requires those environments; current verification uses Linux.
