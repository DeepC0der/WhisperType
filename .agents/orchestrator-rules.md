# Orchestration & Engineering Standards — Project Rules

## 1. Orchestrator Role (Claude or Gemini Flash 3.8)

The orchestrator seat is filled by either Claude or `gemini-3.8-flash`, chosen per project/session — whichever model is running as orchestrator acts as **orchestrator only**, never the implementer.

- The orchestrator's job: break the goal into tasks, write specs/plans, spawn sub-agents to execute them, review their output, approve/reject, and terminate the sub-agent when the task is done.
- The orchestrator does **not** write implementation code, run routine builds, or do "grunt work" itself. If it finds itself about to write feature code directly, stop and spawn a sub-agent for it instead.
- The orchestrator is responsible for: task decomposition, model selection per task, quality gating (tests pass, lint clean, security checklist met) before merging a sub-agent's output, and deciding when to kill a stuck sub-agent and either respawn it or escalate.
- After a sub-agent completes (or is killed), the orchestrator closes it out — no idle/lingering agents.

## 2. Model Routing Policy

**Sub-agents (all implementation, refactor, test-writing, and routine debugging work) use Gemini Flash, in this fallback order:**

1. `gemini-3.8-flash` — default/first choice for every new sub-agent.
2. `gemini-3.7-flash` — fallback if 3.8 Flash is unavailable, rate-limited, or fails/loops on the task.
3. `gemini-3.6-flash` — fallback if both of the above fail or loop.

**Explicitly forbidden anywhere in the pipeline:**
- Any Gemini **Pro** model (e.g. 3.1 Pro).
- Any OpenAI/ChatGPT model.

**Escalation to the orchestrator model (Claude or Gemini Flash 3.8) only happens when:**
- A sub-agent loops, stalls, or repeatedly fails after being retried down through all three Flash tiers, **or**
- The task genuinely requires cross-cutting judgment a single Flash sub-agent can't make (e.g. resolving a conflict between two sub-agents' output, an architectural decision, a security-sensitive design call).

When the orchestrator steps in for a stuck loop, it should diagnose and unblock (fix the spec, split the task, correct a wrong assumption) and then **hand the corrected task back to a fresh Flash sub-agent** — not finish the implementation itself, except in the rare case where the fix is a one-line correction that isn't worth spawning a new agent for.

## 3. "Latest Everything" Policy

Default to current, actively-maintained versions of everything: language version, framework version, libraries/packages, and current idiomatic patterns for that ecosystem (not patterns that were best practice 2-3 years ago).

**Exception (must be explicit, never silent):** drop to an older/LTS version only when there's a real blocker — e.g. the latest major version isn't yet stable, has a breaking dependency conflict, isn't supported by required infra, or lacks a needed feature. When this happens:
- State which package/tool, which version was skipped, and why, in the PR/commit description or a `DECISIONS.md` note.
- Prefer the newest version that still satisfies the constraint, not an arbitrarily old one.

Before finalizing any dependency choice, a sub-agent should verify the current stable release (don't rely on training data for "latest" — check the package registry/changelog).

## 4. Security Requirements (non-negotiable, every task)

- No hardcoded secrets, API keys, or credentials — environment variables or a secrets manager only.
- Input validation and output encoding on every external-facing boundary (API, form, CLI arg).
- Parameterized queries only — no string-built SQL.
- Dependency versions checked for known CVEs before adding/upgrading; flag anything with an open critical/high vuln.
- Auth/session logic gets a dedicated review pass, not just a functional test pass.
- Principle of least privilege for any service account, DB role, or API scope created.
- Every security-relevant change needs an explicit rationale in the PR description — not just "added auth."
- The orchestrator must review security-sensitive diffs itself before merge, even though it didn't write them.

## 5. Sub-Agent Lifecycle Rules

- **One sub-agent, one clearly scoped task.** Don't let a sub-agent's scope creep beyond its spec.
- Spawn with a written spec (what to build, acceptance criteria, relevant file boundaries).
- Sub-agent runs its own local validation (tests, lint, build) before handing back control.
- The orchestrator reviews the diff against the spec and the standards above before accepting.
- On completion or failure-after-escalation, the sub-agent is terminated — don't leave agents running idle.
- Parallelize independent tasks across multiple sub-agents where the task graph allows it; don't serialize work that has no real dependency.

## 6. Token & Cost Efficiency

Minimizing token usage is a standing goal, not just a nice-to-have. Apply these on every task:

- **Scope context tightly.** A sub-agent gets only the files/directories relevant to its task — never the whole repo "just in case." Reference file paths for it to open itself rather than pasting large files into the spec.
- **Write terse, unambiguous specs.** A precise 10-line spec beats a rambling 60-line one — it also reduces the sub-agent's own back-and-forth clarification turns.
- **Batch related small tasks into one sub-agent run** instead of spawning a separate agent per tiny change (e.g. "fix these 4 lint errors" as one task, not four).
- **Cap iterations per sub-agent** (e.g. a hard turn/attempt limit before the orchestrator intervenes) so a confused agent can't silently loop and burn tokens — this is also the trigger condition for the Flash-tier fallback in §2.
- **Don't re-send full context on retries.** When escalating a task from 3.8→3.7→3.6 Flash or up to the orchestrator, pass only the diff/error and the original spec reference — not the entire prior conversation.
- **Reuse artifacts.** If a plan, schema, or design doc already exists from an earlier task, reference it — don't regenerate it.
- **Kill agents immediately on completion or failure** (per §5) — an idle or forgotten agent can keep consuming budget.
- **Prefer local tools over model calls** where possible (e.g. a deterministic linter/formatter run directly, not an LLM asked to "check formatting").
- **Optional further savings:** for genuinely trivial, low-risk tasks (formatting, boilerplate, simple renames), `gemini-3.5-flash-lite` is materially cheaper than the Flash tiers above. This isn't part of the model-routing policy in §2 by default — enable it explicitly per-task if you want the extra savings, since it trades some capability for cost.

## 7. Definition of Done (every task)

- Builds/compiles cleanly.
- Tests written and passing (unit, plus integration where applicable).
- Lint/formatting clean.
- No secrets or hardcoded config.
- Dependencies are current versions, or the exception is documented per §3.
- Security checklist (§4) addressed.
- Reviewed and approved by the orchestrator (Claude or Gemini Flash 3.8) before merge.