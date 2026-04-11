# Runana Reconciliation Inconsistencies

## Purpose

This document compares the revised MVP target in [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L1) against:

- the current on-chain implementation
- the current local documentation in this checkout
- the referenced but currently missing SSOT and settlement-plan docs

For each inconsistency, it proposes explicit reconciliation choices and a recommended path.

## Compared Artifacts

### Present In This Checkout

- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L1)
- [lib.rs](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L1)
- [qs.md](/home/paps/projects/runana-program/programs/runana-program/src/qs.md#L1)
- integration tests under `tests/src`

### Referenced But Missing In This Checkout

- `docs/architecture/SSOT.md`
- `solana-zone-run-execution-and-settlement-plan.md`

## Highest-Level Finding

The revised MVP spec is no longer a small additive layer over the current implementation. It is a deliberate product-and-protocol revision. The largest mismatches are:

1. source-of-truth docs are missing locally
2. settlement player authorization model differs
3. character/account schema differs
4. class system is absent
5. anon/server account model is absent
6. grace-period gameplay intent conflicts with current season-window validation

---

## 1. Missing SSOT And Settlement Plan Docs

### Current State

This checkout does not contain the two docs the project keeps referring to:

- `docs/architecture/SSOT.md`
- `solana-zone-run-execution-and-settlement-plan.md`

### Why This Matters

- We cannot do a real line-by-line reconciliation against the intended prior plan
- The new gap-analysis is currently acting as the temporary de facto product spec
- Any “compare to SSOT/plan” claim would be incomplete until those docs are restored

### Reconciliation Choices

- `Option A`: Restore the missing docs into this repo, then do a second-pass reconciliation
- `Option B`: Treat the new gap-analysis as the temporary SSOT until the old docs are recovered
- `Option C`: Recreate the missing docs from memory/other repos and then reconcile

### Recommendation

- `Recommended`: `Option A` if those files exist elsewhere
- `Fallback`: `Option B` if those docs are no longer authoritative

---

## 2. Settlement Player Authorization Model Mismatch

### Current Implementation

The current settlement flow validates:

- server attestation via ed25519 pre-instruction
- player authorization via a separate signed-message permit

Evidence:

- [lib.rs](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L227)
- [lib.rs](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L1382)
- [lib.rs](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L2067)
- [qs.md](/home/paps/projects/runana-program/programs/runana-program/src/qs.md#L54)

### Revised MVP Spec

The revised MVP requires:

- server attestation remains
- player signed-message permit is removed
- player becomes a real transaction signer
- client builds and submits the transaction
- one Phantom approval target for settlement

Evidence:

- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L106)
- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L752)

### Reconciliation Choices

- `Option A`: Keep current dual-auth model and revise the MVP doc back toward two prompts
- `Option B`: Change the program to keep server attestation but replace player permit with signer validation
- `Option C`: Support both paths temporarily

### Recommendation

- `Recommended`: `Option B`

Why:

- it matches the desired wallet UX
- it preserves server-side settlement attestation
- it removes the extra player message prompt

---

## 3. Character Root Schema Mismatch

### Current Implementation

`CharacterRootAccount` stores:

- authority
- character id
- character creation timestamp

It does not store:

- name
- class id

`CreateCharacterArgs` currently includes only:

- `character_id`
- `initial_unlocked_zone_id`

Evidence:

- [lib.rs](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L574)
- [lib.rs](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L736)

### Revised MVP Spec

The revised MVP requires `name` and `classId` on-chain in the root account.

Evidence:

- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L77)
- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L393)

### Reconciliation Choices

- `Option A`: Keep metadata off-chain and revise the MVP doc
- `Option B`: Expand `CharacterRootAccount` and `CreateCharacterArgs`
- `Option C`: Add a separate metadata PDA instead

### Recommendation

- `Recommended`: `Option B`

Why:

- it matches the revised MVP decisions already locked
- it keeps create atomic
- it avoids a second PDA dependency for basic identity

---

## 4. Class System Missing

### Current Implementation

There is no class registry or class PDA model today.

Existing registries are only:

- zone registry
- zone enemy set
- enemy archetype registry
- season policy

Evidence:

- [lib.rs](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L337)
- [lib.rs](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L360)
- [lib.rs](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L402)
- [lib.rs](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L425)

### Revised MVP Spec

The revised MVP requires:

- one PDA per class
- `classId`
- `enabled`
- admin-controlled enablement

Evidence:

- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L84)
- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L542)
- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L975)

### Reconciliation Choices

- `Option A`: Keep launch classes entirely off-chain
- `Option B`: Add class PDAs now
- `Option C`: Hardcode classes in the program and skip class accounts for MVP

### Recommendation

- `Recommended`: `Option B`

Why:

- it matches the locked MVP decision
- it gives admin control without overloading `CharacterRootAccount`

---

## 5. Level Progression Logic Mismatch

### Current Implementation

Settlement currently updates only:

- `total_exp`
- cursor
- zone/world progress

It does not update `level` from EXP.

Evidence:

- [lib.rs](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L294)
- [lib.rs](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L587)

### Revised MVP Spec

The revised MVP expects:

- EXP-to-level conversion on-chain
- progression tables as program constants

Evidence:

- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L97)
- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L960)

### Reconciliation Choices

- `Option A`: Leave level static for MVP and revise the doc
- `Option B`: Add on-chain level derivation during settlement
- `Option C`: Keep EXP on-chain and derive displayed level only off-chain

### Recommendation

- `Recommended`: `Option B`

---

## 6. Slot Authority Mismatch

### Current Implementation

The on-chain program does not enforce:

- anon 1-slot limit
- wallet 3-slot limit
- slot index assignment

Character PDA derivation is generic by `(authority, character_id)`.

Evidence:

- [lib.rs](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L463)

### Revised MVP Spec

- anon users: exactly 1 slot
- wallet-linked users: exactly 3 slots
- slot assignment is server-only

Evidence:

- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L56)
- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L263)

### Reconciliation Choices

- `Option A`: Move slot semantics on-chain
- `Option B`: Keep slot semantics server-only
- `Option C`: Let the client infer slot order

### Recommendation

- `Recommended`: `Option B`

Reason:

- this is already the locked MVP decision
- no current chain design depends on slot semantics

---

## 7. Anon User Model Missing

### Current Implementation

There is no anon user/account/session model in this repo.

### Revised MVP Spec

- first open auto-creates anon server-backed user
- anon users are real users
- anon and wallet-linked users share the same session model
- wallet linking upgrades the same user identity in the normal case

Evidence:

- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L56)
- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L233)

### Reconciliation Choices

- `Option A`: Make anon local-only again
- `Option B`: Implement full server-backed anon account model
- `Option C`: Hybrid local-first with delayed server user creation

### Recommendation

- `Recommended`: `Option B`

---

## 8. Draft/Finalize Create Model Missing

### Current Implementation

There is no server-side create lifecycle today.

### Revised MVP Spec

Requires:

- name reservation
- class validation
- create draft
- create finalize
- convert draft
- convert finalize

Evidence:

- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L344)
- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L440)

### Reconciliation Choices

- `Option A`: Client-only create without server drafts
- `Option B`: Implement the draft/finalize server contract
- `Option C`: Use drafts for wallet create only, not anon conversion

### Recommendation

- `Recommended`: `Option B`

---

## 9. Settlement Batch/Ack Backend Missing

### Current Implementation

No backend exists today for:

- durable batch records
- attempt records
- txid acknowledgement
- retrying failed unresolved batches

### Revised MVP Spec

Requires:

- one explicit batch record per unresolved settlement group
- multiple attempt records under that batch
- `prepare`
- `ack`
- retry same failed batch

Evidence:

- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L752)

### Reconciliation Choices

- `Option A`: Keep settlement as pure stateless request/submit logic
- `Option B`: Implement durable run -> batch -> attempt backend model
- `Option C`: Track only per-run sync state and skip batches

### Recommendation

- `Recommended`: `Option B`

---

## 10. Share And Public Result Model Missing

### Current Implementation

No share or public result doc/model exists locally.

### Revised MVP Spec

Requires:

- durable server `runId`
- run-scoped result page
- public unlisted share page
- status labels on public pages

Evidence:

- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L650)
- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L710)

### Reconciliation Choices

- `Option A`: Remove share from MVP
- `Option B`: Implement run records + share generation + public result page
- `Option C`: Keep share client-only and non-canonical

### Recommendation

- `Recommended`: `Option B`

---

## 11. Sync Surface Mismatch

### Current Implementation

No sync UI or sync page exists.

### Revised MVP Spec

- per-character sync page
- progression-first summary
- retry sync primary action
- grace-period risk surfaces on roster, character, and sync page

Evidence:

- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L866)
- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L905)

### Reconciliation Choices

- `Option A`: Keep sync as purely background/implicit
- `Option B`: Implement the dedicated sync product surface
- `Option C`: Put sync details only on the character page

### Recommendation

- `Recommended`: `Option B`

---

## 12. Grace-Period Gameplay Semantic Mismatch

### Current Implementation

The current on-chain model enforces:

- battles must occur no later than `season_end_ts`
- submission must occur no later than `commit_grace_end_ts`

Evidence:

- [lib.rs](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L1824)
- [lib.rs](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L1853)
- [lib.rs](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L1867)

### Revised MVP Spec

The revised doc explicitly notes a mismatch if product intent is “normal gameplay continues during grace.”

Evidence:

- [user-flow-spec-gap-analysis.md](/home/paps/projects/runana-program/docs/architecture/user-flow-spec-gap-analysis.md#L928)

### Reconciliation Choices

- `Option A`: Define grace as sync-only in product behavior
- `Option B`: Revise the on-chain season-window logic so grace-time play can still count
- `Option C`: Allow grace-time play, but make those runs non-seasonal/non-settleable

### Recommendation

- `Recommended`: choose explicitly before implementation

Practical default:

- If the goal is lowest risk, choose `Option A`
- If product strongly wants normal gameplay during grace, choose `Option B`

This is the most important unresolved logic conflict.

---

## 13. Current `qs.md` Narrative Is Out Of Date Relative To Revised MVP

### Current Documentation

`qs.md` currently describes the existing chain design accurately, including:

- player-funded character creation
- settlement auth still using trusted server attestation plus player permit

Evidence:

- [qs.md](/home/paps/projects/runana-program/programs/runana-program/src/qs.md#L54)
- [qs.md](/home/paps/projects/runana-program/programs/runana-program/src/qs.md#L60)

### Revised MVP Spec

The revised MVP changes settlement auth and character schema assumptions.

### Reconciliation Choices

- `Option A`: Leave `qs.md` as current-implementation documentation only
- `Option B`: Update `qs.md` after implementation lands
- `Option C`: Add an explicit warning to `qs.md` that it documents pre-revision behavior

### Recommendation

- `Recommended`: `Option C` now, then `Option B` when the implementation changes land

---

## 14. Compatibility Strategy Mismatch

### Current Situation

The revised MVP assumes:

- breaking chain revision is acceptable
- old dev/test data is disposable
- same program id can be upgraded

### Reconciliation Choices

- `Option A`: Preserve compatibility with current experimental data/accounts
- `Option B`: Treat current experimental data as disposable and proceed with breaking MVP revision

### Recommendation

- `Recommended`: `Option B`

---

## Recommended Reconciliation Order

1. Recover or confirm the fate of `SSOT.md` and `solana-zone-run-execution-and-settlement-plan.md`
2. Decide the grace-period gameplay rule explicitly
3. Revise the settlement plan around:
   - server attestation retained
   - player as real signer
   - client-built/client-submitted transaction
4. Revise on-chain character schema for `name` and `classId`
5. Add class PDA design
6. Define server models for:
   - users/accounts
   - character slots
   - name reservations
   - runs
   - settlement batches
   - settlement attempts
7. Add sync/share/result surfaces
8. Update `qs.md` and any restored SSOT/plan docs to match the reconciled design

## Bottom Line

Most inconsistencies are straightforward “current implementation is behind the revised MVP.” The one inconsistency that cannot be safely papered over is the grace-period gameplay semantic conflict. Everything else can proceed once the project accepts the revised MVP as the new target. That grace rule must be reconciled before finalizing the settlement plan.
