# Audit: Tauri Plugin Surface

## Workspace Signal

- Active plugins in `src-tauri/Cargo.toml`: `log`, `dialog`, `fs`, `shell`, `global-shortcut`.

## Complete Technical Blueprint

- Core utility:
  - `log`: desktop log emission.
  - `dialog`: file/folder pickers.
  - `fs`: filesystem bridge.
  - `shell`: shell command / external app bridge.
  - `global-shortcut`: desktop hotkeys.
- Auth flow: local desktop permissions and app capability model, not remote auth.
- Webhooks/events: Tauri event bus, not network webhooks.

## Autonomous Suggestions

- Log all destructive-adjacent UI actions with plugin-level context.
- Keep plugin usage declarative so capability drift is obvious during packaging review.
- Use plugin surfaces to enhance auditability instead of bypassing it with ad hoc shell glue.

## Critical Failings

- Plugin sprawl can create packaging and permissions surprises.
- `shell` and `fs` are power tools; without a strict audit trail they become invisible escape hatches.

## Sources

- https://v2.tauri.app/plugin/dialog/
- https://tauri.app/reference/javascript/fs/
- https://v2.tauri.app/plugin/shell/
- https://v2.tauri.app/reference/javascript/global-shortcut/
