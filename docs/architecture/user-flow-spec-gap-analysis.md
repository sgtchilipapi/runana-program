# Runana MVP User Flow Spec And Gap Analysis

## Purpose

This document defines the desired MVP user flow and compares it against the current repository state so we can perform a practical gap analysis.

It is intended to answer:

- what the MVP user experience should be
- what the server/API contract should be
- what on-chain reads and writes should happen
- where the current implementation or plan must be revised

## Source Of Truth For This Revision

This checkout does not currently contain:

- `docs/architecture/SSOT.md`
- `solana-zone-run-execution-and-settlement-plan.md`

So this revision is based on:

- the user requirements in this thread
- the current on-chain program in `programs/runana-program/src/lib.rs`
- the localnet tests under `tests/src`
- the follow-up product and API decisions made during the interactive planning discussion

## Repo Reality Snapshot

### Implemented Today

- On-chain program config
- On-chain season policy
- On-chain zone registry and zone enemy sets
- On-chain enemy archetype registry
- On-chain character creation
- On-chain settlement batch validation and application
- Strong settlement validation around nonce continuity, batch continuity, server attestation, season windows, zone legality, throughput caps, and cursor advancement

### Missing Today

- Frontend implementation
- Server/API implementation
- Auth/session implementation
- User/account model
- Character metadata model
- Class registry implementation
- Run/session backend
- Sync page and sync backend
- Share page and share backend

## Core MVP Decisions Locked By This Revision

### Product Model

- The app auto-creates an anonymous server-backed user on first open
- Anonymous users are real users, not fake local-only placeholders
- Anonymous users use the same server session model as wallet-linked users
- Anonymous users may create exactly 1 character
- Wallet-linked users may create up to 3 characters
- Wallet connect is available from header/settings and is not the primary landing CTA
- The landing page primary message is instant play, not wallet ownership
- Username/password and SSO are visible only as disabled `coming soon` options

### Identity And Account Model

- Account mode vocabulary is `anon` and `wallet-linked`
- Anon identity is created automatically and persisted through server session plus local continuity
- When an anon user links a wallet, the wallet is attached to the same underlying user account
- If the current anon session differs from an already-known wallet-linked account, the app prompts:
  - `Continue with wallet account`
  - `Stay anon`
- That choice is remembered on the device until changed

### Character Model

- `name` and `classId` are on-chain
- `slotIndex` is server-only
- `name` uniqueness is enforced by the server/DB first, then mirrored on-chain
- Character names are `3-16` characters, ASCII alphanumeric plus spaces
- Rename is not part of MVP
- Rename should be treated as a future extension using the same DB-reserve then on-chain-update model

### Class Model

- Classes are a fixed curated launch set for MVP
- The canonical enabled class set is stored on-chain
- Use one PDA per class
- Each class PDA stores only:
  - `classId`
  - `enabled`
- Program admin controls class enablement
- Class labels, art, and descriptive metadata are served by the backend
- Class does not directly modify settlement outcomes in MVP
- Class is relevant to identity and future derived stat logic

### Progression Model

- EXP-to-level conversion is on-chain
- EXP progression tables should be program constants in MVP
- Derived combat stats remain off-chain in MVP
- Level is on-chain and authoritative

### Settlement UX Revision

- This MVP revision is intentionally breaking relative to the current experimental settlement/player-auth path
- Wallet-backed create should require one Phantom approval
- Settlement should require one Phantom approval
- Settlement remains server-attested
- The server signs the canonical settlement payload
- The player authorizes settlement by being the real transaction signer
- The old player signed-message permit flow is not preserved for MVP
- The server attestation should continue to be verified through an ed25519 pre-instruction
- The player should be validated on-chain as a real signer whose pubkey matches the character owner
- Settlement transactions should be client-built and client-submitted for Phantom compatibility
- Prefer simple legacy transaction shape unless complexity later proves necessary

### Sync Model

- Sync is manual only in MVP
- One sync action prepares one settlement batch
- A settlement batch is a durable server record
- A failed batch remains the same unresolved batch and is retried as that same batch
- Settlement attempts are separate records under the batch
- Batching is oldest-first contiguous grouping of eligible runs
- One sync action settles at most one eligible batch
- If more backlog remains, it stays pending for future sync actions

### Share Model

- Share is available immediately after run end
- Share is the primary post-run action
- Share generation is server-driven
- Public share pages are unlisted and public-by-link
- Public share pages show character name and class, not wallet identity
- Public share pages clearly label `Pending`, `Synced`, or `Expired`
- Expired unsynced runs remain shareable and viewable as expired history

### Grace Period Model

- Unsynced runs become expired at or after grace end
- Expired runs remain viewable read-only
- Expired runs are labeled `Expired, not synced`
- During grace, at-risk progress must be clearly visible on roster, character, and sync pages
- During grace, the character page primary CTA becomes sync-focused
- There is a known design mismatch if grace-period gameplay is intended to continue normally, because the current on-chain settlement model only accepts battles inside the season battle window

## Status Legend

- `Implemented`: present in this repo today
- `Planned`: required by this revised MVP
- `Gap`: missing from this repo today
- `Revision Required`: the current implementation direction must change to match the revised MVP

---

## 1. First Open, Landing, And Session Establishment

### Desired User Story

On first app open:

1. The app silently creates an anonymous server-backed user
2. The server establishes a normal session cookie
3. The app stores local continuity information for that anon identity
4. The user lands on the landing page

### Desired Landing Behavior

- Primary CTA: `Try the Game`
- Wallet connect is visible but secondary
- Wallet connect should live in header/settings, not as the main hero CTA
- Username/password and SSO may be shown as disabled `coming soon` options

### Desired Wallet Auth Contract

#### `POST /api/auth/wallet/challenge`

Request:

- `walletAddress`

Response:

- `challengeId`
- `message`
- `expiresAt`

Behavior:

- Server generates a one-time challenge with expiry
- Client signs the returned message in Phantom

#### `POST /api/auth/wallet/verify`

Request:

- `challengeId`
- `walletAddress`
- `signature`

Response:

- `session`
- `userSummary`
- `accountMode`

Recommended minimum `userSummary`:

- `userId`
- `accountMode`
- `hasAnonCharacter`
- `hasWalletLinked`
- `characterCounts`

### On-Chain Reads/Writes

- None for landing/auth itself

### Current Gap

- `Gap`: no landing page
- `Gap`: no anon auto-user creation flow
- `Gap`: no wallet auth API
- `Gap`: no session implementation

---

## 2. Account Modes And Identity Switching

### Desired User Story

The user can remain anon, link a wallet later, or switch back to the remembered anon identity from account/settings.

### Desired Behavior

- If the current anon session links a fresh wallet, the same user becomes `wallet-linked`
- If the wallet maps to a different existing account, prompt:
  - `Continue with wallet account`
  - `Stay anon`
- If the user signs out of the wallet-linked identity, they return to the remembered anon account if present

### Server Model

- Anonymous and wallet-linked users are both first-class server users
- Wallet linking upgrades identity without requiring a brand new user record in the normal case

### Current Gap

- `Gap`: no server user model
- `Gap`: no account linking flow
- `Gap`: no account/settings switching surface

---

## 3. Character Roster

### Desired User Story

- Anon users have exactly 1 character slot
- Wallet-linked users have exactly 3 character slots

### Desired UI Behavior

#### Anon

- The roster should expose only one usable slot
- If no character exists, show create CTA
- If one character exists, show that character card

#### Wallet-linked

- Always render 3 slots
- Empty slots show `Create Character`
- Occupied slots show compact cards with:
  - `name`
  - `class`
  - `level`
  - `sync badge`

### Server Contract

#### `GET /api/characters`

Response:

- `accountMode`
- `slotsTotal`
- `characters`

Recommended minimum character summary:

- `id`
- `name`
- `classId`
- `level`
- `syncStatus`

Server is authoritative for:

- slot placement
- slot count
- character-to-slot mapping

### On-Chain Reads/Writes

Read-only enrichment for wallet-linked characters:

- `CharacterRootAccount`
- `CharacterStatsAccount`
- `CharacterWorldProgressAccount`
- `CharacterSettlementBatchCursorAccount`

### Current Gap

- `Gap`: no roster UI
- `Gap`: no server slot model
- `Gap`: no anon 1-slot vs wallet 3-slot logic
- `Gap`: no server enrichment/indexing of on-chain character state

---

## 4. Character Creation

### Desired User Story

The user selects a class, enters a unique name, creates a character with a single Phantom approval, and lands on the character page.

### Desired UI Behavior

- Class selection uses cards
- Name field enables after class selection
- Validation states:
  - `empty`
  - `invalid format`
  - `taken`
  - `available`
- Name rule: `3-16 ASCII alnum/space`
- Submit is disabled until class + valid name are present
- If a draft expires, return to the form and revalidate

### Wallet-Linked Create Flow

#### `POST /api/characters/draft`

Request:

- `slotIndex`
- `name`
- `classId`

Response:

- `draftId`
- `characterId`
- `characterRoot`
- `expiresAt`

Server responsibilities:

- validate session/account mode
- validate slot availability
- reserve name with short expiry
- validate enabled class
- derive/create the intended on-chain character identity

Client responsibilities:

- build a simple local Phantom transaction
- submit `create_character`
- wait for confirmation

#### `POST /api/characters/:draftId/finalize`

Request:

- `draftId`
- `txid`
- `characterRoot`

Response:

- `characterSummary`
- `syncSummary`

Finalize rules:

- finalize only after chain confirmation and PDA verification
- if tx fails, keep the draft briefly recoverable

### On-Chain Writes Required

`Revision Required`

The current `create_character` instruction must expand so the revised MVP can store:

- `name`
- `classId`

Planned on-chain writes:

- `CharacterRootAccount`
  - owner
  - character id
  - creation timestamp
  - name
  - class id
- `CharacterStatsAccount`
  - level
  - total EXP
- `CharacterWorldProgressAccount`
- first `CharacterZoneProgressPageAccount`
- `CharacterSettlementBatchCursorAccount`

### Current Gap

- `Implemented`: basic on-chain character creation exists
- `Revision Required`: root account schema must change to include `name` and `classId`
- `Gap`: no server draft/finalize flow
- `Gap`: no name reservation system
- `Gap`: no class registry implementation
- `Gap`: no create UI

---

## 5. Anon Character Creation And Anon-To-Wallet Conversion

### Desired User Story

Anon users can create 1 real server-backed character. Later, if they connect a wallet, that character can convert into wallet-backed slot `1`.

### Anon Creation Behavior

- Anon users are real users
- Anon creation still hits the server for name uniqueness
- Unique global names matter more than offline-first behavior

### Anon Conversion Flow

#### `POST /api/characters/convert-draft`

Request:

- `anonCharacterId`
- `desiredName` when conflict resolution is needed

Response:

- `draftId`
- `characterId`
- `characterRoot`
- `expiresAt`

Rules:

- converted character always occupies wallet slot `1`
- conversion revalidates name uniqueness
- if needed, the user must choose a new unique name before conversion completes

#### `POST /api/characters/convert-draft/:draftId/finalize`

Request:

- `draftId`
- `txid`
- `characterRoot`

Response:

- `characterSummary`
- `syncSummary`

### Conversion Mapping

Carry into the wallet-backed chain character:

- `name`
- `classId`

Do not carry local-only progression directly.

Progression after conversion should continue through normal run settlement.

### Current Gap

- `Gap`: no anon character server model
- `Gap`: no anon conversion draft/finalize flow
- `Gap`: no wallet-link upgrade behavior

---

## 6. Character Page

### Desired UI Behavior

Top-of-page emphasis:

- identity + progression first

Recommended visible fields:

- `name`
- `class`
- `level`
- progression summary
- season summary

Sync behavior:

- always-visible sync button on character page
- compact sync summary row in active season
- during grace, sync becomes the primary CTA if at-risk progress exists

### Server Contract

#### `GET /api/characters/:characterId`

Response:

- `characterSummary`
- `progressionSummary`
- `seasonSummary`
- `syncSummary`

### On-Chain Reads

- `CharacterRootAccount`
- `CharacterStatsAccount`
- `CharacterWorldProgressAccount`
- relevant `CharacterZoneProgressPageAccount` pages
- `CharacterSettlementBatchCursorAccount`

### Current Gap

- `Gap`: no character page
- `Gap`: no sync summary model
- `Gap`: no season summary API

---

## 7. Class Catalog

### Desired Backend Model

- Fixed curated launch set
- Enabled set is on-chain authoritative
- Server reads enabled class PDAs and serves the current catalog to clients

### Recommended API

#### `GET /api/classes`

Response:

- `classes`

Each class item should contain off-chain metadata at minimum:

- `classId`
- `displayName`
- `artKey`
- `description`
- `enabled`

### On-Chain Writes Required

`Gap`

Need a class registry instruction set, likely:

- initialize class
- enable/disable class

Using one PDA per class.

### Current Gap

- `Gap`: no class PDAs
- `Gap`: no class admin flow
- `Gap`: no class catalog API

---

## 8. Run Setup Page

### Desired UI Behavior

- Zone selection is the primary focus
- Current season timing is visible
- Locked zones should appear as teaser cards
- Anon and wallet-linked users use the same basic run setup structure

### Season Display Rules

- Active season:
  - show countdown to season end
- Grace period:
  - show countdown to grace end
  - emphasize finishing sync for this season

### Recommended APIs

#### `GET /api/seasons/current`

Response:

- `seasonId`
- `seasonNumber`
- `seasonName`
- `seasonStartTs`
- `seasonEndTs`
- `commitGraceEndTs`
- `phase`

#### `GET /api/characters/:characterId/run-context`

Response:

- `characterSummary`
- `seasonSummary`
- `availableZones`
- `progressionSummary`

### On-Chain Reads

- `SeasonPolicyAccount`
- `CharacterWorldProgressAccount`
- relevant `CharacterZoneProgressPageAccount` pages

### Current Gap

- `Implemented`: season policy timing exists on-chain
- `Gap`: season presentation metadata layer
- `Gap`: run setup UI
- `Gap`: zone teaser and selection UI

---

## 9. Run Execution And Result Presentation

### Desired Run Map Behavior

This revision should not change the existing zone run gameplay flow or established zone map progress stepper logic.

The change is presentation-only:

- show only previous/current/immediate-next local context
- local window only for the stepper/map presentation
- do not hardcode a new branch-count rule in this document
- follow the established zone design for immediate-next options

### Run Backend Model

- Server creates a durable run record at run start
- `runId` is durable and server-generated
- Run result summary is server-evaluated before sync

### Recommended API

#### `POST /api/runs`

Request:

- `characterId`
- `zoneId`

Response:

- `runId`
- `characterSummary`
- `zoneSummary`
- `seasonSummary`
- `initialRunState`

### Result Page

- Use a run-scoped route
- Result page remains share-first in active season
- In active season, sync should appear only as a low-emphasis path to the sync page
- During grace, result sharing remains available, but the broader app surfaces at-risk state more strongly

### Recommended API

#### `GET /api/runs/:runId`

Response:

- `runId`
- `characterSummary`
- `resultSummary`
- `status`
- `shareState`

Status vocabulary:

- `Pending`
- `Synced`
- `Expired`

### Current Gap

- `Gap`: run start API
- `Gap`: run record model
- `Gap`: result page
- `Gap`: server-evaluated result pipeline

---

## 10. Share Flow

### Desired Behavior

- Share is primary in post-run UX
- Share is available immediately after run end
- Share is server-generated from the canonical run record

### Recommended API

#### `POST /api/runs/:runId/share`

Response:

- `shareUrl`
- `shareText`
- `status`

Share URL target:

- public run result page

Public share page should show:

- character name
- class
- run outcome
- status label

Public share discoverability:

- unlisted public-by-link

### Current Gap

- `Gap`: share generation endpoint
- `Gap`: public run result page
- `Gap`: share storage/model

---

## 11. Sync And Settlement

### Desired MVP Settlement Contract

`Revision Required`

Target model:

1. Server prepares or reuses the current unresolved batch for a character
2. Server returns canonical batch payload, server attestation, and account metas
3. Client builds the local Phantom transaction
4. Server attestation is included and verified through ed25519 pre-instruction
5. Player signs the transaction once
6. Client submits the transaction
7. Client immediately acknowledges the `txid` back to the server
8. Server tracks attempts and confirmation state

### Recommended APIs

#### `POST /api/characters/:characterId/settlement/prepare`

Request:

- `characterId`

Response:

- `batchId`
- `runIds`
- `status`
- `payload`
- `serverAttestation`
- `accountMetas`

If no eligible backlog exists:

- return `200` with noop/empty result

#### `POST /api/characters/:characterId/settlement/ack`

Request:

- `runId`
- `txid`

Response:

- `batchId`
- `attemptId`
- `status`

### Sync Retry Behavior

- Retry targets the failed unresolved batch
- Reuse the same unresolved batch if still valid
- A batch remains durable across multiple attempts

### Batch Model

- batch identity is explicit and durable on the server
- batch statuses:
  - `Prepared`
  - `Submitted`
  - `Confirmed`
  - `Failed`
  - `Expired`
- attempts are separate child records under a batch

### Batching Rules

- oldest-first contiguous pending runs
- one batch per sync tap
- partial backlog remains pending after success
- use server-recorded run completion order as queue authority

### On-Chain Validation Target

Keep:

- server attestation verification
- canonical batch validation
- nonce continuity
- batch continuity
- season checks
- cursor checks
- zone legality

Change:

- remove separate player message permit requirement
- require player authority as real signer

### On-Chain Writes Planned

- total EXP
- level derived from EXP
- world progress
- zone progress
- settlement cursor

### Current Gap

- `Implemented`: current canonical settlement core exists
- `Revision Required`: player auth model must change
- `Revision Required`: transaction flow must become client-built and client-submitted
- `Gap`: settlement prepare API
- `Gap`: ack API
- `Gap`: batch and attempt server models
- `Gap`: sync retry UI and backend

---

## 12. Dedicated Sync Page

### Scope

- per character

### Desired Behavior

- In active season, sync remains mostly a dedicated page concern
- The roster shows only a compact badge
- The character page has a visible sync button plus summary
- Retry sync is the primary action on the sync page

### Recommended API

#### `GET /api/characters/:characterId/sync`

Response:

- `characterSummary`
- `progressionSummary`
- `syncSummary`
- `batches`
- `attempts`

Recommended top-of-page emphasis:

- character progression first
- sync state second

Recommended primary summary:

- latest confirmed progression state
- whether unresolved work exists
- latest unresolved batch state

### Current Gap

- `Gap`: dedicated sync page
- `Gap`: sync summary model
- `Gap`: batch/attempt listing model

---

## 13. Grace Period And Expiry

### Desired User Behavior

- Grace is the last chance to settle old seasonal progress
- At-risk state should be visible on:
  - roster
  - character page
  - sync page

### Expiry Model

- At or after grace end, unsynced runs become expired
- Expired runs remain viewable read-only
- Expired runs cannot be settled
- Use player-facing wording:
  - `Expired, not synced`

### Server Model

- Use a server-side expiry job at or after grace end

### Known Design Mismatch

If product intent is that players should keep doing normal season-equivalent runs during grace, the current chain season-window model must be revised.

Why:

- current on-chain settlement validates battle timestamps against the season battle window
- grace currently behaves like delayed commit time, not like extended playable season time

This is not a minor UI issue. It is a settlement-plan revision point.

### Current Gap

- `Gap`: expiry job
- `Gap`: grace warning UX
- `Revision Required`: settlement plan must explicitly resolve grace-period gameplay semantics

---

## 14. On-Chain Schema And Instruction Revisions Required

### Character Root

`Revision Required`

Add:

- `name`
- `classId`

### Character Stats

`Revision Required`

Keep:

- `level`
- `totalExp`

Change:

- level should be derived on-chain from EXP progression constants

### Class Registry

`Gap`

Add one PDA per class with:

- `classId`
- `enabled`

### Settlement Instruction

`Revision Required`

Keep:

- server ed25519 attestation verification path

Replace:

- player signed-message permit verification

With:

- player authority real signer requirement

---

## 15. Recommended Example API Surface

- `POST /api/auth/wallet/challenge`
- `POST /api/auth/wallet/verify`
- `GET /api/classes`
- `GET /api/seasons/current`
- `GET /api/characters`
- `POST /api/characters/draft`
- `POST /api/characters/:draftId/finalize`
- `POST /api/characters/convert-draft`
- `POST /api/characters/convert-draft/:draftId/finalize`
- `GET /api/characters/:characterId`
- `GET /api/characters/:characterId/run-context`
- `GET /api/characters/:characterId/sync`
- `POST /api/runs`
- `GET /api/runs/:runId`
- `POST /api/runs/:runId/share`
- `POST /api/characters/:characterId/settlement/prepare`
- `POST /api/characters/:characterId/settlement/ack`

---

## 16. Acceptance Priorities

### Highest UX Acceptance Criteria

- wallet-backed character creation uses one Phantom approval
- settlement uses one Phantom approval
- create and settle both result in confirmed server+chain state

### Highest Product Acceptance Criteria

- users can try instantly as anon
- anon-to-wallet upgrade works without identity confusion
- users clearly see at-risk unsynced progress during grace

### Highest Health Signals

- gameplay KPI: completed runs per active user
- sync KPI: prepared-to-confirmed batch success rate
- identity KPI: anon-to-wallet upgrade conversion

### Recovery Expectations

- drafts are recoverable while valid
- failed batches remain retryable
- ack failure surfaces as non-blocking pending-sync notice while retry continues
- expired states remain inspectable rather than disappearing silently

---

## 17. Gap Analysis Summary

| Area | Desired MVP | Current Repo |
| --- | --- | --- |
| Landing + anon first-open | Auto anon user, session cookie, try-first UX | Missing |
| Wallet auth | Challenge + verify flow | Missing |
| Account switching | Anon vs wallet-linked account choice in settings | Missing |
| Roster | Anon 1-slot, wallet 3-slot, server slot authority | Missing |
| Character metadata | Name + class on-chain, slot server-side | Not implemented |
| Name uniqueness | DB reserve then mirror on-chain | Missing |
| Class registry | On-chain class PDAs + server catalog | Missing |
| Create flow | Draft -> one Phantom tx -> finalize | Partially chain-only today |
| Anon conversion | Convert anon character into slot 1 | Missing |
| Character page | Identity/progression summary + sync access | Missing |
| Run setup | Zone-first setup with season countdown and teaser locked zones | Missing |
| Run records | Durable server run ids and result pages | Missing |
| Share | Immediate server-generated share + public run page | Missing |
| Settlement UX | Server attests, player tx-signs, client submits once | Not implemented |
| Settlement auth | Remove player message permit, require real signer | Revision required |
| Sync backend | Durable batch + attempt model | Missing |
| Sync page | Per-character, progression-first, retry batch | Missing |
| Grace expiry | Expired read-only unsynced history | Missing |
| Grace gameplay semantics | Must be explicitly resolved in settlement plan | Unresolved |

## Conclusion

The current repository already contains a solid nucleus: canonical on-chain settlement validation. But the revised MVP defined here is no longer just “add UI around the existing plan.” It requires deliberate revision in four major places:

1. the account model, because anon users are now real server-backed users from first open
2. the character model, because `name` and `classId` move on-chain
3. the settlement UX, because player auth changes from signed message permit to transaction signer
4. the season/grace model, because normal gameplay during grace conflicts with the current settlement time-window rules unless the plan is revised

This document should now be used as the updated baseline for the next settlement-plan revision and implementation sequencing.
