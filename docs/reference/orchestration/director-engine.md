# Director Engine
> Core orchestration loop that receives track tasks, plans acquisition strategies, waterfalls through providers, and finalizes validated candidates into the music library.

**Status:** Proven Working
**Code:** `crates/cassette-core/src/director/engine.rs`

## What It Does

The Director Engine is the central coordinator of the Cassette download pipeline. It accepts track acquisition requests through an mpsc channel, plans a provider strategy for each task, then executes a two-pass waterfall across all configured providers to find, validate, score, and finalize audio files into the library.

Each submitted task gets a dedicated temporary workspace managed by TempManager. The engine enforces global worker concurrency via a semaphore and per-provider concurrency via individual provider semaphores. Results and progress events are broadcast to subscribers in real time, enabling the UI to track every stage from queue entry through finalization or failure.

On startup the engine runs stale temp recovery to clean up incomplete work from previous sessions. Failed tasks can optionally preserve their quarantine directories for debugging. The engine shuts down gracefully when the submission channel is dropped, draining all in-flight tasks before returning.

## Key Types

```rust
pub struct DirectorSubmission {
    tx: mpsc::Sender<TrackTask>,
}
// Methods: submit(task) -> Result<(), DirectorError>

pub struct DirectorHandle {
    pub submitter: DirectorSubmission,
    pub events: broadcast::Sender<DirectorEvent>,
    pub results: broadcast::Sender<DirectorTaskResult>,
    manager: tokio::task::JoinHandle<Result<(), DirectorError>>,
}
// Methods: subscribe() -> Receiver<DirectorEvent>
//          subscribe_results() -> Receiver<DirectorTaskResult>
//          shutdown() -> Result<(), DirectorError>

pub struct Director {
    config: DirectorConfig,
    providers: Vec<Arc<dyn Provider>>,
    planner: StrategyPlanner,
}

enum ProviderAttemptOutcome {
    Busy,                                          // semaphore full, defer to pass 2
    Tried,                                         // attempted, no usable result
    Finalized(FinalizedTrack, Vec<ProviderAttemptRecord>),  // immediate winner
}
```

## How It Works

### Startup

`Director::start()` creates three channels:
- `mpsc::channel::<TrackTask>(128)` for task submission
- `broadcast::channel::<DirectorEvent>(256)` for progress events
- `broadcast::channel::<DirectorTaskResult>(256)` for completion results

It then spawns the `run()` loop as a tokio task and returns a `DirectorHandle`.

### Run Loop

1. Calls `recover_temp()` to clean stale temp directories from prior sessions
2. Creates a global `Semaphore` from `worker_concurrency` and builds per-provider semaphores from `provider_policies`
3. Receives `TrackTask` messages from the mpsc channel
4. Sends a `Queued` event, acquires a global semaphore permit
5. Spawns `process_task` into a `JoinSet`
6. When the channel closes, drains all remaining tasks from the JoinSet

### process_task Flow

1. Collects `ProviderDescriptor` from all providers
2. Plans strategy via `StrategyPlanner::plan()`
3. If strategy is `MetadataRepairOnly` -- sends `MetadataOnly` disposition and returns
4. Creates a temp workspace via `TempManager::prepare_task()`
5. Calls `execute_waterfall()`
6. On success: sends `Finalized` result, cleans temp directory
7. On failure: sends `Failed` result (or `AlreadyPresent` for `DestinationExists`), preserves quarantine if `quarantine_failures` is enabled

### Two-Pass Waterfall (execute_waterfall)

**Pass 1 (non-blocking):** Iterates `provider_order` from the strategy plan. For each provider, calls `try_provider` with `blocking=false`. If the provider semaphore is full, the provider is added to a `deferred_providers` list. If `FirstValidWins` mode produces an immediate finalization, returns immediately.

**Pass 2 (blocking):** Iterates `deferred_providers`. Calls `try_provider` with `blocking=true`, waiting for the semaphore.

**Candidate selection:** After both passes, filters candidates by `temp_path` existence (to handle cases where quarantine invalidated a shared path), then selects the candidate with the highest `score.total`.

### try_provider Flow

1. Checks `supports_download` capability -- skips metadata-only providers
2. `execute_provider_search` (with semaphore + retry) to get search candidates
3. For each search candidate: `execute_provider_acquire` (with semaphore + retry)
4. `validate_candidate` on the acquired file
5. If `require_lossless` and candidate is not Lossless -- quarantine and continue
6. `score_candidate` to compute the 6-factor score
7. If `FirstValidWins` and not `collect_multiple_candidates` -- finalize immediately
8. Otherwise: push to `valid_candidates` for later comparison

### finalize_candidate

1. Re-scores the candidate to generate a `SelectionReason`
2. Builds a `CandidateSelection` with score, validation, temp_path, and cover_art_url
3. Sends `Tagging` event, calls `apply_metadata` (non-fatal on error)
4. Sends `Finalizing` event, builds `ProvenanceRecord`
5. Calls `finalize_selected_candidate` to move the file into the library

### Retry Logic (execute_with_retry)

- `max_attempts` from `retry_policy.max_attempts_per_provider` (minimum 1)
- Each attempt wrapped in `timeout()` using `provider_timeout_secs`
- Exponential backoff: `base_backoff_millis * attempt_number`
- Only retries errors where `retryable()` returns true (RateLimited, TimedOut, Network, TemporaryOutage)
- Non-retryable errors fail immediately

### Provider Semaphores

Built from `provider_policies` in config. Each provider gets its own `Arc<Semaphore>` with `max_concurrency` permits. Non-blocking acquisition uses `try_acquire_owned()`, blocking uses `acquire_owned()`.

### Event Broadcasting

`DirectorEvent` contains: `task_id`, `progress` (DirectorProgress enum), `provider_id` (optional), `message`.

Progress states flow: `Queued` -> `InProgress` -> `ProviderAttempt` -> `Validating` -> `Tagging` -> `Finalizing` -> `Finalized`/`Failed`/`Exhausted`/`Skipped`

## Configuration

| Setting | Default | Description |
|---|---|---|
| `worker_concurrency` | 12 | Maximum concurrent tasks via global semaphore |
| `provider_timeout_secs` | 45 | Timeout per provider operation (search or acquire) |
| `retry_policy.max_attempts_per_provider` | 2 | Maximum attempts before giving up on a provider |
| `retry_policy.base_backoff_millis` | 500 | Base backoff duration; multiplied by attempt number |
| `duplicate_policy` | KeepExisting | How to handle files already at the destination path |
| `temp_recovery.quarantine_failures` | true | Preserve quarantine dirs for failed tasks |
| `temp_recovery.stale_after_hours` | 24 | Hours before temp dirs are considered stale |
| `provider_policies[id].max_concurrency` | 1 | Per-provider concurrency limit |

## Code Map

| File | Role |
|---|---|
| `crates/cassette-core/src/director/engine.rs` | Director, DirectorHandle, DirectorSubmission, run loop, waterfall, retry logic (~780 lines + ~310 lines tests) |
| `crates/cassette-core/src/director/strategy.rs` | StrategyPlanner -- maps AcquisitionStrategy to StrategyPlan with provider ordering |
| `crates/cassette-core/src/director/scoring.rs` | score_candidate -- 6-factor scoring function |
| `crates/cassette-core/src/director/validation.rs` | validate_candidate -- Symphonia-based audio validation |
| `crates/cassette-core/src/director/finalize.rs` | finalize_selected_candidate -- file placement and deduplication |
| `crates/cassette-core/src/director/temp.rs` | TempManager -- per-task temp directories and stale recovery |
| `crates/cassette-core/src/director/metadata.rs` | apply_metadata -- writes tags to the validated candidate |
| `crates/cassette-core/src/director/config.rs` | DirectorConfig, QualityPolicy, RetryPolicy, TempRecoveryPolicy, ProviderPolicy |
| `crates/cassette-core/src/director/models.rs` | All shared types: TrackTask, DirectorEvent, DirectorTaskResult, CandidateDisposition, etc. |
| `crates/cassette-core/src/director/error.rs` | DirectorError, ProviderError, ValidationError, FinalizationError |
