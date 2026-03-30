# codex-relay-rs — Handoff Artifact (What Is Left)

This document summarizes remaining work after the initial scaffold and v1 local relay flow implementation.

## Current Status (Implemented)
- Typed internal schema (`TransportMessage`, `RelayJob`, `AgentRequest`, `AgentResponse`, payloads).
- Core relay loop (`poll_once`) with policy check and single active job lock.
- In-memory transport + local Telegram adapter simulation helpers.
- Agent abstraction + noop adapter + basic registry structs.
- Artifact filesystem with job-scoped dirs and zip extract/re-zip helpers.
- Security policy primitives (allowlist, payload constraints).
- `.env.example` with Telegram + agent/channel binding placeholders.

---

## Remaining Work

### 1) Real Telegram Integration (Highest Priority)
- Replace local-only `TelegramAdapter` enqueue helpers with real Bot API polling/webhook client.
- Implement update offset persistence and robust reconnect/backoff.
- Implement outbound API calls (`sendMessage`, `sendDocument`, media endpoints).
- Map Telegram file IDs to downloaded files and artifact store ingestion.

### 2) App Entry Point + Runtime Wiring
- Add executable binary (`src/bin/...` or `app` crate) that wires:
  - config loading
  - policy engine
  - transport adapter
  - artifact root
  - selected agent adapters
- Add run loop and graceful shutdown handling.

### 3) Config System
- Add typed config structs and loader (`env` + optional file).
- Parse:
  - Telegram token/API values
  - chat allowlist
  - per-agent channel bindings
  - artifact limits
  - default agent

### 4) Agent Execution (Real Codex Adapter)
- Replace noop adapter with real Codex process/client adapter.
- Pass workspace path and artifacts to Codex execution context.
- Handle streaming output and cancellation semantics.

### 5) Security Hardening
- Enforce per-agent permissions during routing/execution (currently shaped but not fully enforced).
- Add strict MIME sniffing + extension checks (not only declared MIME).
- Add zip extraction safeguards for total expanded size / file count limits.
- Add rate limiting and abuse prevention controls.

### 6) State & Reliability
- Persist sessions/jobs and update offsets (SQLite/Postgres/Redis).
- Add retry policies (transport send failures, transient agent failures).
- Add timeout and cancellation propagation across adapters.

### 7) Observability
- Replace `eprintln!` with structured `tracing` logs.
- Add metrics counters/timers (jobs started/completed/failed, artifact bytes, latency).
- Add correlation IDs through all logs and adapter boundaries.

### 8) Testing
- Unit tests for:
  - policy authorization + routing
  - zip-slip protection
  - payload validation
- Integration tests for:
  - text flow (telegram -> relay -> agent -> reply)
  - zip flow (upload -> extract -> rezip -> outbound)

### 9) Multi-Agent Orchestration (Future)
- Dynamic routing strategy (capability + load-based).
- Parallel fan-out coordination and response aggregation.
- Per-agent queue/concurrency controls.

### 10) Packaging / Repo Structure (Optional but Recommended)
- Consider moving from single crate module layout to multi-crate workspace:
  - `relay-core`
  - `transport-telegram`
  - `agent-codex`
  - `artifact-store`
  - `policy`
  - `app`

---

## Suggested Next Milestone
**Milestone: “Telegram Real IO + Runnable App”**
1. Add config loader.
2. Implement real Telegram polling + outbound send.
3. Wire runnable binary loop.
4. Add integration test for text echo flow.

Deliverable: a runnable local relay that can receive Telegram messages and return agent responses.
