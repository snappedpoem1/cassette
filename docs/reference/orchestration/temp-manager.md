# Temp Manager
> Per-task temporary directory management with quarantine support and automatic stale recovery on Director startup.

**Status:** Proven Working
**Code:** `crates/cassette-core/src/director/temp.rs`

## What It Does

The TempManager provides isolated temporary workspaces for each download task. Every task gets its own directory tree with an `active/` subdirectory for in-progress downloads and a `quarantine/` subdirectory for files that failed validation. A JSON marker file tracks when each workspace was created, enabling automatic cleanup of stale directories from crashed or interrupted sessions.

The quarantine mechanism serves a dual purpose: it prevents failed downloads from cluttering the active workspace while preserving them for debugging or manual inspection. When `quarantine_failures` is enabled, failed files are moved to quarantine rather than deleted. The stale recovery process respects this by preserving quarantine directories even when deleting the rest of an expired task workspace.

On every Director startup, `recover_stale()` runs to clean up leftovers from previous sessions. Workspaces older than `stale_after_hours` are either deleted entirely or have their quarantine directories preserved based on policy.

## Key Types

```rust
#[derive(Debug, Clone)]
pub struct TempManager {
    root: PathBuf,
    policy: TempRecoveryPolicy,
}

#[derive(Debug, Clone)]
pub struct TaskTempContext {
    pub task_id: String,
    pub root: PathBuf,           // {root}/{safe_task_id}/
    pub active_dir: PathBuf,     // {root}/{safe_task_id}/active/
    pub quarantine_dir: PathBuf, // {root}/{safe_task_id}/quarantine/
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TempMarker {
    task_id: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TempRecoverySummary {
    pub deleted_roots: Vec<PathBuf>,
    pub preserved_quarantine: Vec<PathBuf>,
}

pub struct TempRecoveryPolicy {
    pub stale_after_hours: u32,      // default: 24
    pub quarantine_failures: bool,   // default: true
}
```

## How It Works

### Task ID Sanitization (safe_dir_name)

Converts a task ID into a safe directory name by replacing filesystem-unsafe characters with `-`:

Replaced characters: `: * ? " < > | \ /`

After replacement, trailing dots and spaces are stripped (Windows rejects directory names ending with `.` or ` `).

### prepare_task

```rust
pub async fn prepare_task(&self, task_id: &str) -> Result<TaskTempContext, std::io::Error>
```

1. Computes the task root: `{self.root}/{safe_dir_name(task_id)}/`
2. Creates `{root}/active/` and `{root}/quarantine/` directories via `create_dir_all`
3. Writes `active.json` marker to the task root containing `{ task_id, created_at }` as pretty-printed JSON
4. Returns a `TaskTempContext` with all paths populated

### cleanup_task

```rust
pub async fn cleanup_task(&self, context: &TaskTempContext) -> Result<(), std::io::Error>
```

Removes the entire task root directory recursively via `remove_dir_all`. Called by the engine after successful finalization.

### quarantine_file

```rust
pub async fn quarantine_file(
    &self,
    context: &TaskTempContext,
    source: &Path,
) -> Result<PathBuf, std::io::Error>
```

Moves a file from the active directory (or wherever it is) into the quarantine directory. Preserves the original filename; defaults to `"candidate.bin"` if the filename cannot be determined. Uses `tokio::fs::rename`.

### recover_stale

```rust
pub async fn recover_stale(&self) -> Result<TempRecoverySummary, std::io::Error>
```

Called on Director startup via `Director::recover_temp()`:

1. Ensures the temp root directory exists
2. Iterates all subdirectories of the root
3. For each directory, reads the `active.json` marker
4. Computes a cutoff time: `now - stale_after_hours`
5. If the marker is missing or `created_at < cutoff`, the workspace is stale
6. For stale workspaces:
   - If `quarantine_failures` is true and a `quarantine/` subdirectory exists: preserves the quarantine dir, adds to `preserved_quarantine` list
   - Otherwise: deletes the entire task root via `remove_dir_all`, adds to `deleted_roots` list
7. Returns `TempRecoverySummary` with both lists

### Directory Layout

```
{temp_root}/
  {task-id-1}/
    active.json          # TempMarker { task_id, created_at }
    active/              # in-progress downloads land here
      candidate.flac
    quarantine/          # failed validations moved here
      candidate.mp3
  {task-id-2}/
    active.json
    active/
    quarantine/
```

## Configuration

| Setting | Default | Description |
|---|---|---|
| `temp_root` | (from DirectorConfig) | Root directory for all task temp workspaces |
| `temp_recovery.stale_after_hours` | 24 | Hours before a temp workspace is considered stale and eligible for cleanup |
| `temp_recovery.quarantine_failures` | true | Preserve quarantine directories for failed tasks instead of deleting everything |

## Code Map

| File | Role |
|---|---|
| `crates/cassette-core/src/director/temp.rs` | TempManager, TaskTempContext, TempMarker, TempRecoverySummary (131 lines code + 24 lines tests) |
| `crates/cassette-core/src/director/config.rs` | TempRecoveryPolicy struct with stale_after_hours and quarantine_failures |
| `crates/cassette-core/src/director/engine.rs` | Consumer: creates TempManager in process_task, calls prepare_task/cleanup_task/quarantine_file; calls recover_temp on startup in run() |
