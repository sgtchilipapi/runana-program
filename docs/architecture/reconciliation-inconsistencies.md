# Runana Reconciliation Inconsistencies

## Purpose

This document compares the revised MVP target in [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L1) against:

- the current on-chain implementation in `runana-program`
- the current local implementation notes in [qs.md](/home/paps/projects/runana-program/programs/runana-program/src/qs.md#L1)
- the actual project SSOT in `keep-pushing`
- the actual existing zone-run execution and settlement plan in `keep-pushing`
- the current deferred-settlement API spec in `keep-pushing`

For each inconsistency, this report gives reconciliation choices and a recommended direction.

## Compared Artifacts

### Revised MVP Spec

- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L1)

### Current Program Repo

- [lib.rs](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L1)
- [qs.md](/home/paps/projects/runana-program/programs/runana-program/src/qs.md#L1)

### Multi-Root Workspace Docs In `keep-pushing`

- [SSOT.md](/home/paps/projects/keep-pushing/docs/architecture/SSOT.md)
- [solana-zone-run-execution-and-settlement-plan.md](/home/paps/projects/keep-pushing/docs/architecture/solana/solana-zone-run-execution-and-settlement-plan.md)
- [deferred-settlement-api-spec.md](/home/paps/projects/keep-pushing/docs/api/deferred-settlement-api-spec.md)

## Highest-Level Finding

The revised MVP doc and the existing `keep-pushing` docs agree on some foundational principles:

- server-side combat authority
- on-chain bounded-legality validation rather than exact-path cryptographic truth
- player-owned Solana actions funded by the player wallet
- season/grace semantics where delayed submission is allowed but prior-season progress can expire

But they diverge sharply on four large areas:

1. gameplay model
   - `keep-pushing` canonical direction is run-native zone traversal
   - revised MVP doc still describes a simpler run/result flow without adopting the full run-native sealing model
2. settlement transport
   - existing API spec is server-prepared opaque transaction, two-phase authorize -> sign_transaction -> submit
   - revised MVP wants client-built, client-submitted, one-prompt settlement
3. local-first onboarding model
   - existing API spec is local-first character creation and first-sync atomic chain bootstrap
   - revised MVP now treats anon users as real server-backed users immediately and pushes more identity/value on-chain earlier
4. batching unit
   - zone-run plan says settlement unit is a whole closed run and runs are never split
   - current on-chain program is battle-native
   - revised MVP doc does not yet fully adopt the whole-closed-run settlement unit

The most important reconciliation question is:

- do we preserve the existing `keep-pushing` run-native + first-sync architecture and revise the new MVP doc back toward it,
- or do we deliberately replace that architecture with the revised MVP direction and update the old docs accordingly?

---

## 1. SSOT Trust Model vs Revised MVP Settlement UX

### SSOT

The SSOT says:

- all combat simulation is server-side only
- EXP claims are never direct server/client inputs
- player-owned on-chain actions are funded by the player wallet
- server may prepare and broadcast player-signed transactions
- broadcast origin is not an on-chain validity condition

Source:

- [SSOT.md](/home/paps/projects/keep-pushing/docs/architecture/SSOT.md)

### Revised MVP Doc

The revised MVP says:

- settlement remains server-attested
- player becomes the real transaction signer
- settlement should be client-built and client-submitted for Phantom UX
- one Phantom approval is the target

### Inconsistency

This is not a hard contradiction.

The SSOT allows server-prepared or server-broadcast transactions, while the revised MVP prefers client-built/client-submitted transactions. The real difference is implementation preference, not trust-model incompatibility.

### Reconciliation Choices

- `Option A`: Keep SSOT trust model and allow both client-submit and server-broadcast operationally
- `Option B`: Tighten SSOT to explicitly prefer client-built/client-submitted settlement for MVP UX
- `Option C`: Revise the MVP doc back toward server-prepared opaque transactions

### Recommendation

- `Recommended`: `Option B`

Why:

- it keeps the SSOT security model intact
- it aligns the spec with the one-approval Phantom UX goal

---

## 2. Existing Deferred-Settlement API Spec vs Revised MVP Settlement Flow

### Existing API Spec

The current API spec in `keep-pushing` describes:

- anonymous user creation via `POST /api/auth/anon`
- local-first backend character creation via `POST /api/character/create`
- real combat persistence before chain existence
- atomic first sync:
  - prepare authorize phase
  - player signs authorization message
  - prepare sign-transaction phase
  - server returns opaque `preparedTransaction`
  - wallet signs prepared transaction
  - backend submit endpoint broadcasts and reconciles
- later settlement mirrors the same authorize -> sign_transaction -> submit pattern

Source:

- [deferred-settlement-api-spec.md](/home/paps/projects/keep-pushing/docs/api/deferred-settlement-api-spec.md)

### Revised MVP Doc

The revised MVP says:

- no separate player signed-message permit
- client prepares settlement locally from structured server data
- client submits transaction directly
- client then acknowledges `txid`
- settlement should be one Phantom approval

### Inconsistency

This is a hard architecture divergence.

The current API spec is built around:

- opaque server-prepared transactions
- player signed-message permits
- backend submit endpoints

The revised MVP is built around:

- structured prepare data
- signer-based player auth
- client-side submission
- ack endpoint instead of submit endpoint

### Reconciliation Choices

- `Option A`: Keep the existing deferred-settlement API architecture and revise the MVP doc
- `Option B`: Replace the current API spec with the revised MVP settlement architecture
- `Option C`: Support both flows temporarily

### Recommendation

- `Recommended`: `Option B`

Why:

- it directly addresses the reported Phantom UX problem
- it removes the second player authorization prompt
- it keeps server attestation while simplifying player interaction

### Required Doc Follow-Up

If `Option B` is chosen, these `keep-pushing` docs must be revised:

- [deferred-settlement-api-spec.md](/home/paps/projects/keep-pushing/docs/api/deferred-settlement-api-spec.md)
- any backend plan that assumes `preparedTransaction` opaque relay submission

---

## 3. First-Sync Local-First Model vs Revised MVP Account Model

### Existing API Spec

Current spec says:

- create backend-only anon user
- create backend-only playable character immediately
- play local battles immediately
- later perform atomic first sync that creates chain character and settles batch 1

### Revised MVP Doc

Revised MVP says:

- anon users are real server-backed users from first open
- anon users can have 1 character
- wallet-linked users can have 3 characters
- anon-to-wallet conversion upgrades the same user
- name and class should be on-chain identity fields

### Inconsistency

These models overlap in spirit but differ in emphasis:

- existing spec centers local-first gameplay before chain identity
- revised MVP centers unified server account identity plus earlier chain-oriented character identity

The largest concrete mismatch is onboarding shape:

- existing spec uses one backend character with chain status `NOT_STARTED`
- revised MVP uses anon/wallet-linked account mode and formal slot semantics

### Reconciliation Choices

- `Option A`: Preserve current local-first-first-sync model and revise the new MVP doc to fit it
- `Option B`: Keep local-first play, but adopt the revised account model and slot semantics on top of it
- `Option C`: Remove the local-first-first-sync architecture and push chain-oriented creation earlier

### Recommendation

- `Recommended`: `Option B`

Why:

- it preserves the strongest part of the current app direction: immediate play
- it still allows the revised account/slot model to exist cleanly

Practical interpretation:

- anon users are real server users
- they still get immediate backend-playable characters
- wallet conversion becomes the chain bootstrap point

---

## 4. Run-Native Settlement Unit vs Revised MVP Batch Model

### Zone-Run Plan

The zone-run plan is explicit:

- settlement unit is a closed settleable run, not an individual battle
- batches contain contiguous ranges of settleable closed runs
- no run may ever be split across two batches
- zero-value runs do not enter settlement continuity

Source:

- [solana-zone-run-execution-and-settlement-plan.md](/home/paps/projects/keep-pushing/docs/architecture/solana/solana-zone-run-execution-and-settlement-plan.md)

### Current Program

The current on-chain implementation is battle-native:

- `battle_count`
- `start_nonce` / `end_nonce`
- encounter histogram at batch level
- zone progress delta at batch level

Source:

- [lib.rs](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L736)

### Revised MVP Doc

The revised MVP talks about runs, run ids, and batching, but it still references the current battle-native program and does not yet fully rewrite the settlement payload around closed-run summaries.

### Inconsistency

This is the biggest architecture mismatch after the player-auth change.

The revised MVP doc and the zone-run plan are not yet fully aligned on the settlement unit. The zone-run plan is much more specific and should win unless deliberately superseded.

### Reconciliation Choices

- `Option A`: Keep battle-native settlement and revise the zone-run plan back
- `Option B`: Adopt the zone-run plan fully and update the MVP doc to closed-run-native settlement
- `Option C`: Ship an interim battle-native MVP and plan a later run-native migration

### Recommendation

- `Recommended`: `Option B`

Why:

- the zone-run plan is marked design-locked and canonical for that workstream
- the revised MVP doc should be updated to explicitly adopt closed-run summaries, zero-value run rules, and no-run-splitting

---

## 5. Gameplay API Family Mismatch

### Zone-Run Plan

Canonical gameplay API family is:

- `POST /api/zone-runs/start`
- `GET /api/zone-runs/active`
- `POST /api/zone-runs/choose-branch`
- `POST /api/zone-runs/advance`
- `POST /api/zone-runs/use-skill`
- `POST /api/zone-runs/continue`
- `POST /api/zone-runs/abandon`

The current direct encounter route may remain only as sandbox/testing behavior.

### Revised MVP Doc

The revised MVP still uses generalized routes like:

- `POST /api/runs`
- `GET /api/runs/:runId`

and does not fully describe the canonical mutating run action family.

### Inconsistency

This is a real contract mismatch between the canonical zone-run workstream and the new doc’s simplified route examples.

### Reconciliation Choices

- `Option A`: Keep the simplified `/api/runs` family and revise the zone-run plan
- `Option B`: Update the MVP doc to use the canonical `/api/zone-runs/*` action family
- `Option C`: Keep `/api/runs` as a read/result/share family while using `/api/zone-runs/*` for execution

### Recommendation

- `Recommended`: `Option C`

Why:

- it preserves the richer canonical zone-run action model
- it still allows stable run-id-based result/share pages

---

## 6. Topology Visibility Agreement vs Presentation Detail

### Zone-Run Plan

The player sees:

- current node
- legal next branches
- not the full future graph

Topology visibility is adjacent-only.

### Revised MVP Doc

The revised MVP says:

- preserve the existing zone run flow
- presentation-only local window
- previous/current/immediate-next context
- local-window stepper

### Inconsistency

This is mostly compatible, but the revised MVP adds a “previous node” presentation concept that the zone-run plan does not discuss.

### Reconciliation Choices

- `Option A`: Treat previous-node visibility as harmless presentation detail
- `Option B`: Remove previous-node wording from the MVP doc and use strict adjacent-only language
- `Option C`: Update the zone-run plan to mention previous-node context as allowed presentation

### Recommendation

- `Recommended`: `Option A`

This is a cosmetic/spec wording difference, not a logic conflict.

---

## 7. Grace-Period Gameplay Semantics Conflict

### SSOT

SSOT canonical settlement dictionary says:

- delayed submission is valid
- prior-season uncommitted progress expires after grace
- commit within grace or lose uncommitted prior-season progress

### Current Program

Current chain validation enforces:

- `last_battle_ts <= season_end_ts`
- submission time must still be within `commit_grace_end_ts`

### Revised MVP Discussion

The revised MVP notes a mismatch if product expects normal play to continue during grace.

### Zone-Run Plan

The zone-run plan says:

- a run is bound to the season active at run start
- it may not continue once that season's playable window closes
- season-cutoff closure is treated like abandon for settlement semantics

### Inconsistency

The zone-run plan and current chain behavior align well: grace is for settlement delay, not for continued normal seasonal play.

The only inconsistency comes from the product desire expressed during planning that “same as normal” gameplay might continue during grace.

### Reconciliation Choices

- `Option A`: Freeze grace as sync/closure-only and update product wording
- `Option B`: Redesign season playable window semantics across backend + chain + docs
- `Option C`: Allow non-seasonal grace gameplay only

### Recommendation

- `Recommended`: `Option A`

This now has a stronger recommendation than before, because SSOT and the zone-run plan both support it.

---

## 8. Player Fee-Payer Rule vs Client Submit UX

### SSOT

SSOT says:

- player-owned on-chain actions are funded by the player wallet

### Current Program

Current `create_character` enforces:

- payer must equal authority

Current settlement does not check fee payer.

### Revised MVP Doc

The revised MVP also assumes:

- wallet-backed create uses player wallet
- settlement is client-submitted and player-signed

### Inconsistency

No meaningful inconsistency here.

The only difference is operational preference:

- create must remain player-funded
- settlement may be client-submitted while still satisfying the SSOT trust model

### Reconciliation Choices

- `Option A`: Keep as-is
- `Option B`: Extend settlement to require player fee payer too

### Recommendation

- `Recommended`: `Option A`

Do not add unnecessary settlement fee-payer restrictions unless product explicitly wants that burden.

---

## 9. `qs.md` Narrative vs Target Architecture

### Current `qs.md`

`qs.md` accurately describes the current implementation, including:

- player-funded character creation
- settlement authorization still coming from trusted server attestation plus player permit

Source:

- [qs.md](/home/paps/projects/runana-program/programs/runana-program/src/qs.md#L54)
- [qs.md](/home/paps/projects/runana-program/programs/runana-program/src/qs.md#L60)

### Inconsistency

`qs.md` is now out of date relative to the revised MVP target and also out of date relative to the likely future run-native redesign.

### Reconciliation Choices

- `Option A`: Keep `qs.md` as implementation-only documentation until code changes land
- `Option B`: Add a warning that it documents the pre-revision protocol
- `Option C`: Rewrite it now toward the target design

### Recommendation

- `Recommended`: `Option B`, then `Option A` until implementation changes

Do not rewrite `qs.md` toward target behavior before the code changes, or it will stop documenting reality.

---

## 10. Existing Deferred-Settlement Status Vocabulary vs Revised MVP Sync Model

### Existing API Spec

Current status vocabulary includes:

- character chain status: `NOT_STARTED`, `PENDING`, `SUBMITTED`, `CONFIRMED`, `FAILED`
- battle ledger status: `AWAITING_FIRST_SYNC`, `LOCAL_ONLY_ARCHIVED`, `PENDING`, `SEALED`, `COMMITTED`
- settlement batch status: `SEALED`, `PREPARED`, `SUBMITTED`, `CONFIRMED`, `FAILED`

### Revised MVP Doc

The revised MVP uses:

- player-facing run status: `Pending`, `Synced`, `Expired`
- batch status: `Prepared`, `Submitted`, `Confirmed`, `Failed`, `Expired`

### Inconsistency

This is mostly naming divergence plus the fact that the current API spec is battle/first-sync-centric while the revised MVP is run/result-centric.

### Reconciliation Choices

- `Option A`: Keep internal statuses as-is and map them to simpler player-facing labels
- `Option B`: Normalize everything to the new simpler vocabulary
- `Option C`: Keep battle-first and run-first vocabularies in parallel

### Recommendation

- `Recommended`: `Option A`

Use:

- simple player-facing labels in UI and public docs
- richer internal statuses in backend workflows

---

## 11. Character Metadata Placement vs Existing API Spec

### Existing API Spec

Current character create endpoint is:

- `POST /api/character/create`
- request: `{ userId, name? }`

It assumes backend-only local-first character creation and does not model on-chain `classId`.

### Revised MVP Doc

Revised MVP requires:

- `name` and `classId` on-chain
- slot-aware server drafts/finalize

### Inconsistency

This is a real API and persistence model mismatch.

### Reconciliation Choices

- `Option A`: Keep backend-only character creation and add chain metadata only at first sync
- `Option B`: Move create/convert flows to the new draft/finalize model and revise the old API spec
- `Option C`: Add compatibility wrappers around the old endpoints

### Recommendation

- `Recommended`: `Option B`

If `Option B` is chosen, the old `POST /api/character/create` spec should be retired or relabeled as prototype/local-first legacy.

---

## 12. Run History / Share Pages vs Existing Docs

### Existing Zone-Run Plan

The zone-run plan is focused on:

- active run execution
- closure
- settlement sealing

It does not define public share pages as a primary architectural concern.

### Revised MVP Doc

The revised MVP adds:

- durable run-scoped result pages
- public unlisted share pages
- expired-but-viewable history

### Inconsistency

This is additive, not contradictory.

### Reconciliation Choices

- `Option A`: Treat share/result pages as product-layer additions on top of the run-native plan
- `Option B`: Remove share from MVP

### Recommendation

- `Recommended`: `Option A`

---

## 13. Compatibility Strategy

### Current Situation

The revised MVP assumes:

- breaking revision is acceptable
- old experimental/test data is disposable
- same program id can be upgraded

The `keep-pushing` docs do not establish a stronger contrary compatibility requirement.

### Reconciliation Choices

- `Option A`: Breaking revision with disposable non-production data
- `Option B`: Preserve compatibility with current local-first/deferred-settlement artifacts

### Recommendation

- `Recommended`: `Option A`

---

## Recommended Reconciliation Order

1. Accept the multi-root docs in `keep-pushing` as the real prior source docs.
2. Decide whether the revised MVP is intended to replace the current deferred-settlement API architecture.
3. If yes, update these `keep-pushing` docs first:
   - [deferred-settlement-api-spec.md](/home/paps/projects/keep-pushing/docs/api/deferred-settlement-api-spec.md)
   - [SSOT.md](/home/paps/projects/keep-pushing/docs/architecture/SSOT.md) only if you want to explicitly prefer client submission
4. Update the revised MVP doc to fully adopt the zone-run plan’s closed-run settlement unit and `/api/zone-runs/*` execution family.
5. Keep grace as sync/closure-only unless you intentionally redesign the season model across backend and chain.
6. Revise the on-chain program around:
   - signer-based player auth
   - root metadata fields
   - class PDAs
   - on-chain EXP-to-level derivation
   - eventually run-native payload redesign
7. After implementation starts, update [qs.md](/home/paps/projects/runana-program/programs/runana-program/src/qs.md#L1) with a warning that it documents pre-revision behavior.

## Bottom Line

After comparing against the real SSOT and zone-run docs in `keep-pushing`, the situation is clearer:

- the revised MVP doc is directionally compatible with the SSOT trust model
- it is not yet fully compatible with the current deferred-settlement API architecture
- it is not yet fully compatible with the run-native settlement unit described in the canonical zone-run plan

The recommended reconciliation is:

1. keep the SSOT security model,
2. keep the run-native zone-run direction,
3. replace the old two-phase opaque prepared-transaction settlement UX with the revised one-prompt signer-based player flow,
4. update both the new gap-analysis and the old `keep-pushing` docs to meet in that reconciled middle.
