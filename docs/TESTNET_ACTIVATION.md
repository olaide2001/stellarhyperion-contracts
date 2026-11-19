# Current Stellar Testnet deployment

The wallet-owned router and ITS adapter were deployed with the dedicated `hyperion-testnet-20261028` CLI wallet and activated after the normal 3600-second router timelock. This wallet owns the fresh deployment; it does not control the older October 1 router.

| Setting          | Confirmed value                                              |
| ---------------- | ------------------------------------------------------------ |
| Router           | `CCXPCT3YFLVANSF3AQMYCCZVGCMLTURAYQ6O6DYGQJMTKVYQL6NTOC2V`   |
| CCTP adapter     | `CARH6IA4TA5QAJNQG4B4I7N4336JEBS3GYLPZSEBUCWSNYTDRC7L3AHT`   |
| ITS adapter      | `CDFYEPIXO676W6E6Z4GOOR33JMRUC6DGUBGO4OWYKTO7HOGRJ6ARNOSJ`   |
| Admin / treasury | `GDXRQQETEZHGRJTE3U7DYGIWGGYGMM6EAS5NWGPGN2WXFXZYZ6QZIANA`   |
| Stellar USDC SAC | `CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA`   |
| Destination      | `sepolia` / Axelar `ethereum-sepolia`                        |
| Received token   | `0xFd5a146686d16060ef9c595B37C30b248402304c`, seven decimals |
| Activated        | `2026-10-28T17:08:25Z`                                       |

The received asset is the ITS representation of Stellar USDC, not Circle's native Sepolia USDC. Route 1 sends directly to the recipient through ITS and needs no EVM Hyperion adapter.

## Deployment evidence

`deployments/stellar-testnet-phase1.json` is the canonical app/indexer record, retaining its historical filename. `deployments/evidence/2026-10-28-paid-its.json` records actual transaction hashes, ledgers, artifact hashes, token mapping, lane link, queued actions, execution and transfer acceptance status. Check that evidence for the latest source/destination verification result.

The current router uses WASM `281d946ea78b194c0ef44ad9488ed02b2aa6912fb4f6fbab523ad38f1e0803f1`; the CCTP adapter uses `80f109e7467ace6899f865a4f77079917a2bea8a5def593a4d87c08c09206af4`, built from forwarding implementation `b91c6ca`. The unchanged ITS adapter uses `5f32139554d29edc3501a39ae3d05761101234523dd48b4d9e2d81ebf001f2c3`, built from `60b3310`. Actual deployed code was downloaded and its SHA-256 verified. All initial configuration actions and the later CCTP/router upgrade actions were confirmed after their original 3600-second timelocks matured. No shortened timelock or bootstrap bypass was used.

The older deployment is preserved in `deployments/archive/stellar-testnet-2026-10-01.json`. Its separate admin upgrade instructions are historical in `docs/LEGACY_TESTNET_UPGRADE.md`. Do not run those old-address commands against this fresh deployment.

## Verified acceptance transfer

A real CLI wallet signature through the frontend application helpers sent 0.1 USDC. Stellar confirmed source transaction `b95ca748421ea4774dea6a08edebc3ad3a301244034cd10f1209f3cea234862d`. The protocol fee was 0.0003 USDC; 0.0997 USDC arrived on Sepolia in transaction `0x1fdb81fcdec18071adbdf47e359f0b9e4adc6be685c1e3bfcfb39689485037b6`. Independent RPC reads confirmed the recipient token balance and successful receipt; the backend verified matching ITS/payment events and persisted delivered status. The live acceptance test passed.

## Active Circle CCTP and native USDC

CCTP route 0 is active as of `2026-10-29T10:03:44Z`. The adapter uses Circle's published Stellar Testnet messenger and transmitter. Domains 0, 6 and 26 link to Ethereum Sepolia, Base Sepolia and Arc Testnet. Actions 5 through 7 configured and enabled CCTP; action 8 upgraded the router first. All are executed. The existing ITS lane and adapter are preserved.

`deployments/evidence/2026-10-29-cctp.json` retains preparation, upgrade, timelock and activation evidence plus real delivered acceptance transfers on all three destinations. Each test sent 0.3 testnet USDC; the protocol fee was deducted before the remaining 0.2991 USDC was burned, and the forwarding fee was deducted on destination. Confirmed native-USDC payouts were 0.038033 on Sepolia, 0.240087 on Base Sepolia and 0.276441 on Arc Testnet. Forwarding fees vary with destination gas and amount; these are historical observed results. The application displays a live budget and receive minimum before signing. Six-decimal native Circle USDC differs from the seven-decimal ITS representation.

Paid dispatch uses Circle's `cctp-forward` hook and standard finality. The router enforces the minimum destination amount after the full forwarding budget. Circle handles the destination mint without a Hyperion EVM signer. Backend verification binds the attested message to the source transfer and requires matching finalized `MessageReceived` and native-USDC mint events in a successful, confirmed canonical destination receipt. A forwarding API status alone is insufficient.

`script/stellar/enable-cctp.py` provides resumable `queue`, `forwarding` and `activate` stages. This deployment is already active; do not requeue or redeploy it. The tool keeps confirmed steps atomically and never bypasses the timelock.

## Liquid Stellar assets

The frontend discovers classic Stellar assets and finds real Horizon conversion paths into canonical USDC. Asset listing does not guarantee liquidity. Conversion is a separate, capped strict-receive transaction, followed by a fresh bridge review and signature. Converted USDC stays in the user's wallet if bridging cannot proceed. Public checkpoints preserve actual transaction hashes across interruption and prevent blind resubmission.

`deployments/evidence/2026-10-29-conversion.json` records an actual fresh native-XLM wallet, USDC trustline creation and conversion, followed by an independently verified ITS payout of 0.8107937 USDC on Sepolia. The original ITS acceptance remains preserved above. Never repeat any delivered proof transaction.

## Running the application

Use Node 22.13 or newer and a Stellar CLI identity held outside Git. The configured admin identity is named above; private key material must remain in the CLI store. Application users connect their own Stellar Testnet wallets and need testnet XLM for reserves/network fees and a liquid source balance. Direct USDC sends need the canonical trustline/balance; conversion can create that trustline. ITS also needs its displayed XLM relay budget.

Run the backend with PostgreSQL, the canonical deployment record, indexing and rail polling enabled. Redis is required only for an enabled keeper. Start Next with its backend API origin configured. The frontend verifies the live router, mapping, selected destination token and paid-transfer methods before allowing a signature.

The frontend's opt-in `test/live-transfer.test.ts` uses the actual application quote and submission helpers with a named CLI wallet. It spends 0.1 testnet USDC plus relay/network fees, saves the real source hash before broadcast, and checks destination token balance and verified indexer delivery. It is skipped in normal test runs. Set these public/non-secret inputs only when intentionally running a live test:

```bash
HYPERION_LIVE_TESTNET=1 \
HYPERION_TESTNET_IDENTITY=hyperion-testnet-user-20261028 \
HYPERION_TESTNET_RECIPIENT=0x6ef8032ffE4943F27388172549f6700f18BB8b38 \
NEXT_PUBLIC_HYPERION_API_URL=http://127.0.0.1:8080 \
HYPERION_TESTNET_APP_URL=http://127.0.0.1:3100 \
HYPERION_TESTNET_PROOF=/tmp/hyperion-live-transfer-proof.json \
npx vitest run test/live-transfer.test.ts
```

Preserve the proof file across interrupted runs. Never delete a pending source hash and blindly send again. A confirmed Stellar transaction alone does not establish destination delivery. Require the indexer's matching confirmed bridge/token payment and the recipient's actual received-token balance. The additional opt-in `test/live-routes.test.ts` covers native XLM conversion and CCTP destination payouts; see the frontend README for its explicit route/proof settings.

For future contract changes, use the router's existing queue/execute upgrade path with its current admin and timelock. Adapter upgrades require the adapter admin. Mapping/link setters can overwrite configuration; read current values and confirm the intended token/network before signing.

## Configuration-diff proposals

`script/stellar/propose.sh` compares a desired deployment manifest with the live router and prints unsigned proposals. It is the reviewable handover between reading the chain and queueing an action; it is not an admin client.

```bash
# Canonical active manifest; read-only against the testnet RPC.
script/stellar/propose.sh

# A proposed manifest, kept as a handover artifact.
script/stellar/propose.sh path/to/desired-manifest.json --out /tmp/proposals.json

# Offline rehearsal against a local snapshot (no `stellar` binary required).
script/stellar/propose.sh deployments/stellar-testnet-phase1.json \
  --fixture script/stellar/test/fixtures/drift
```

Every chain access is a simulated view call (`--is-view` / `--send no`). There is no signing flag, no `--send yes`, and no `queue_action` call in this path. The source account named by `STELLAR_IDENTITY` (default `hyperion-deploy`) is only the transaction source for simulation. Output on stdout is a JSON object with `differences`, `proposals`, `alreadyQueued`, `guardianActions`, `signed: false`, and `submitted: false`; a matching live state yields empty arrays and a non-zero exit is reserved for a refused generation.

Generation aborts before diffing when the live passphrase is not `Test SDF Network ; September 2015`, when the manifest's passphrase disagrees with the chain, or when the deployed router WASM hash differs from `wasmHashes.router`. Adapter hashes are checked when present too. This is what stops a proposal built from this record being used against the archived October 1 router or a mainnet deployment. Manifests whose `family` is not `stellar` are also refused: EVM and other rail proposal generation stays explicitly gated until that side is independently validated.

Only real differences become proposals: config fields (`admin`, `guardian`, `treasury`, `feeBps`, `flowWindow`, `timelockDelay`), token registration or a flow-limit increase, router-side adapter/receiver/enable wiring, and an optional `routeConfigs` entry with `maxSingleTransfer`. An i128 limit is carried as an exact decimal string throughout; no proposal is produced for a value that already matches, and an identical action already sitting in the queue is reported in `alreadyQueued` rather than emitted again. A flow-limit decrease or route disable is a guardian/immediate action, not a timelocked proposal, because it only narrows what the bridge can do. A changed token decimal count aborts generation instead of proposing a re-registration that would silently overwrite it.

Each proposal carries its `delay` requirement: the desired `delaySeconds` plus the router's `minDelaySeconds`, `maxDelaySeconds` and `gracePeriodSeconds`. The operator still has to queue with `queue_action` under the current admin and execute only after the delay matures, as documented above. The tool never shortens that wait and never bypasses the timelock.

**Compatibility and migration.** The command targets the action variants and integer widths of the deployed router hash recorded above; it does not upgrade or modify that contract. If the router is upgraded, update the manifest's `wasmHashes.router` in the same change that updates this runbook, regenerate proposals, and re-check the wire format against `hyperion-router/src/types.rs` and the protocol parity suite. Adapter `link` setters and adapter ownership remain a separate authority and are intentionally absent from router proposals; they are still performed by their own scripts after reading the live peer/domain settings. Keeping a proposal file is safe after execution: re-running the command against the settled state produces an empty diff and submits nothing.

Offline fixtures for the guards and drift cases live in `script/stellar/test/fixtures`; run the suite with `node --test script/stellar/test/propose.test.mjs`.

The proposal planner refuses unreadable state rather than treating an RPC failure as absent
configuration. Only the router's explicit `TokenNotRegistered`, `AdapterNotSet`, and
`TimelockNotQueued` errors represent missing entries. Reads use `--send=no`, a 30-second
per-command deadline and a 4 MiB response limit. More than 10,000 historical queue IDs
requires a separately reviewed bounded inventory instead of an unbounded scan. The ledger
close timestamp excludes expired actions from deduplication; an expired action must be queued
again. Proposal delays report the current live timelock delay, including when proposing a
change to that delay. All desired router and adapter code hashes are mandatory; the canonical
`adapter-axelar` record supplies the `axelar-its` hash. Numerical JSON amounts above JavaScript's
safe range must be decimal strings, and contract-width limits are checked before output.

## Storage upkeep, restoration planning, schema migration, and action audit

### Archived storage restoration planner (#47)

Restoration is planned off chain. A Soroban guest cannot prove archival from key presence:
allocating a sequence number does not prove a record still exists or is archived. Guest reads of archived entries may reject or restore during
simulation; an archived instance cannot run a guest planner at all. No guest API guesses archive
state. The RPC planner works even when the instance/code itself needs restoration.

Use the RPC planner before upkeep, migration, or settlement:

```sh
node script/stellar/restore-plan.mjs https://soroban-testnet.stellar.org recorded-request.json
```

The request contains `keys` (1–32 canonical base64 `LedgerKey` XDR values from a recorded
footprint) and `transaction` (the original invocation envelope XDR). The planner only calls
`simulateTransaction` and `getLedgerEntries`. It signs and submits nothing. It proves archival
only from `restorePreamble` or CAP-0066 read-write archive indices in transaction data, reports active keys from their ledger TTL, reports a missing key
only when a successful matching simulation touched it without a restore request, and otherwise
reports unknown. For manual restoration it emits the exact complete restore footprint, including required code or
instance dependencies, the original Soroban transaction data XDR and the exact decimal resource
fee. CAP-0066 automatic restoration emits its exact indexed keys and original transaction data,
without fabricating a separate restore operation. Footprints over 32 keys are refused. Review the footprint, restore it, then re-simulate the
original invocation; fee and ledger evidence are estimates for that simulation, not a submission
guarantee. No recipient is inferred from a key, claim number, caller, or supplied metadata.

The local archived-claim snapshot fixture expires a genuine persistent ledger entry, applies a
TTL-only restoration to that recorded entry, reloads its unchanged payload, and settles to the
recorded recipient exactly once. This fixture models the ledger restoration operation; it does
not claim to submit a live network restore transaction.

### Versioned storage schema migration (#46)

`get_migration_status.current_version` reports 2 for new/migrated routers, implicit 1 for initialized legacy
storage, and 0 for an uninitialized router. Financial record layouts and all amount/recipient
fields are unchanged. Version 2 adds a fixed migration state. Record version is determined from
its family cursor and captured high-water mark: migrated IDs and new IDs above the captured
mark use the target version; remaining legacy IDs use the current version. These cursor values
are metadata, not proof that a missing record exists. This avoids an independently expiring
schema sidecar and an unbounded map in instance storage.

After the ordinary reviewed WASM upgrade, queue and execute `BeginStorageMigration(2)` through
the existing admin timelock. This captures fixed transfer/claim high-water marks and authorizes
bounded work. `migrate_storage(admin, 2, max_steps)` accepts 1–32 steps only after that delayed
authorization. It decodes and rewrites each old typed record exactly and commits its version cursor atomically. Missing or archived records must be restored first: they never
advance the cursor. Readers support old and new records while interrupted, new deliveries do
not change captured totals, repeated authorization never resets progress, and calls after
completion return the same completed status. This migration never guesses a balance or changes
a payout, fee, flow window, recipient, or replay identifier. Future layout changes need a new
reviewed version and conversion, rather than treating this v2 conversion as a generic migration.

### Timelock cancellation reasons and action audit metadata (#44)

Soroban `cancel_action_with_reason` and gated EVM `cancelActionWithReason` accept the six
structured reason codes and at most 128 **printable ASCII bytes (0x20–0x7e)**. Unicode, control
characters and oversized notes reject before any queue mutation. Legacy cancellation signatures
remain and use an empty note with `Unspecified`.

Existing queue/execution/cancellation events keep their original encoding. New Soroban audit
and migration events, and EVM audit events, explicitly carry schema version 1.
Soroban retains the original `QueuedAction` payload and schedule in `ResolvedAction` storage and
exposes `get_action_payload`; cancelled actions leave the executable
queue. Versioned immutable audit events record the actor for each transition. A pre-audit
queuing author remains unknown; the cancellation event identifies only its resolver. EVM retains
the canonical `queuedAction` payload and schedule after cancellation, exposes `actionCancelled`,
and records versioned lifecycle metadata (actor, status, reason and note) in
`ActionAuditUpdated` events. These immutable events are the EVM audit source; there is no second
copy of the dynamic action or note in storage. Consumers combine the retained payload with the
versioned events. Execution and recancellation both reject cancelled actions.

### Adapter upkeep bounds, coverage and exclusions (#49)

Every adapter exposes `key_coverage`. Verification reads the actual registered
adapter and queries its own coverage. Upkeep permits at most 32 persistent key visits in total
(excluding the single instance/instance extension), with conservative duplicate counting and
Cartesian products included. Every input vector is separately bounded; unused families reject
non-empty inputs instead of silently pretending to maintain them. TTL maintenance changes no
mapping values or administration and requires no admin authorization.

| Adapter   | Maintained persistent key families                                                                                                                                                      | Exclusions                                                                                                           |
| --------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| CCTP      | Domain/Chain/Caller peers; Asset(domain, remote token)                                                                                                                                  | Rail-owned nonce state; local token lists and inbound-message lists are unsupported                                  |
| Axelar    | Link/Source peers; TokenId/Token forward and reverse mappings                                                                                                                           | ITS-owned replay/manager state; numeric domains and inbound-message lists are unsupported                            |
| Allbridge | Lane/Chain peers; Asset(token, chain); Processed(message)                                                                                                                               | Remote-token list unsupported; gated inbound support remains unchanged                                               |
| Router    | Bounded `keep_alive`: route/token configuration, route limits/windows, claim/transfer payloads; existing `was_processed` and the new action view extend the supplied replay/action keys | Call each registered adapter directly for upkeep and `key_coverage`; verification reads the registered address first |

The Soroban contract spec embeds public Rust documentation in the WASM. Public API documentation
uses concise, complete summaries; full explanations remain in ordinary source comments and this
runbook. Guest exports, parameter/result types, event layouts and runtime behavior are preserved.
The same pinned Stellar CLI builds these artifacts locally and in CI under the unchanged 65,536-byte
ledger gate.

The EVM compiler uses 1,000 optimizer runs to retain EIP-170 deployment headroom; the same profile
is used locally and by full CI, with exact gas reports. The deployment size gates are unchanged.

The adapter instance carries configuration, gas settings and the Allbridge validator; extending
it preserves those fields together. External rail contracts need their own restoration/upkeep
and are never implicitly maintained by these entrypoints. Expiration fixtures for all three
adapters verify maintained peer mappings remain usable past their previous expiry with the same
values and admin.

### Compatibility, migration, and rail gating

The active deployment remains **Stellar Testnet outbound ITS/CCTP**. All previous `AdminAction`
variants, error numbers, rail/message formats, `ActionCancelled` events, `keep_alive` and legacy
cancellation signatures remain. `BeginStorageMigration` is an appended Soroban-only variant.
The read-only proposal tool does not generate upgrades or migrations automatically. Verification
requires readable v2 schema/migration/actual-adapter views; unavailable RPC views never silently
prove legacy v1 or completed migration. ABI artifacts
for the additive EVM audit API are regenerated from Forge output. No deployment record changes
and no live activation occur here. EVM, mainnet and non-active rails remain gated pending their
independent acceptance validation. Adapter upgrade authority remains its existing admin; the
router's reviewed timelock does not silently grant control over external adapter admins.
