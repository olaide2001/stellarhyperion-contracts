#!/usr/bin/env bash
# Development rehearsal on an already running local Anvil node. All accounts are unlocked locally.
set -euo pipefail
cd "$(dirname "$0")/.."

export RPC="${RPC:-http://127.0.0.1:8545}"
for tool in forge cast node; do command -v "$tool" >/dev/null; done
chain=$(cast chain-id --rpc-url "$RPC")
[[ "$chain" == 31337 || "$chain" == 1337 ]] || { echo 'local rehearsal only' >&2; exit 1; }
bash script/anvil-e2e.sh

previous=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
next=0x90F79bf6EB2c4f870365E785982E1f101E93b906
export HYPERION_HANDOVER_NEXT_OWNER="$next"
export HYPERION_RECORD="deployments/evm-$chain-phase1.json"
json() { node -e 'const fs=require("node:fs");process.stdout.write(String(JSON.parse(fs.readFileSync(process.argv[1],"utf8"))[process.argv[2]]))' "$1" "$2"; }
record="evm/$HYPERION_RECORD"
cctp=$(json "$record" cctpAdapter)
axelar=$(json "$record" axelarAdapter)
recipient=$(json "$record" stellarRailRecipient)

handover() {
  local mode="$1" sender="$2"
  ( cd evm && HYPERION_HANDOVER_MODE="$mode" forge script script/OwnershipHandover.s.sol --rpc-url "$RPC" --broadcast --unlocked --sender "$sender" )
}
refuse_config() {
  local sender="$1"
  if cast call "$cctp" 'setLane(string,uint32,bytes32)' ownership-check 27 "$recipient" --from "$sender" --rpc-url "$RPC" >/dev/null 2>&1; then
    echo 'unauthorized CCTP configuration accepted' >&2; exit 1
  fi
  if cast call "$axelar" 'setPeer(string,string,bytes)' ownership-check ownership-check-axelar "$recipient" --from "$sender" --rpc-url "$RPC" >/dev/null 2>&1; then
    echo 'unauthorized ITS configuration accepted' >&2; exit 1
  fi
}

handover report "$previous"
cp "evm/deployments/ownership-$chain-report.json" "evm/deployments/ownership-$chain-initial.json"
handover nominate "$previous"
cp "evm/deployments/ownership-$chain-nominate.json" "evm/deployments/ownership-$chain-pending.json"
handover nominate "$previous"
refuse_config "$next"

# Accept just one contract first; the report must show two different actual authority states.
cast send "$cctp" 'acceptOwnership()' --rpc-url "$RPC" --unlocked --from "$next" >/dev/null
handover report "$previous"
cp "evm/deployments/ownership-$chain-report.json" "evm/deployments/ownership-$chain-partial.json"
handover accept "$next"
cp "evm/deployments/ownership-$chain-accept.json" "evm/deployments/ownership-$chain-accepted.json"
handover accept "$next"
handover nominate "$previous"
handover report "$previous"
refuse_config "$previous"

cast send "$cctp" 'setLane(string,uint32,bytes32)' ownership-check 27 "$recipient" --rpc-url "$RPC" --unlocked --from "$next" >/dev/null
cast send "$axelar" 'setPeer(string,string,bytes)' ownership-check ownership-check-axelar "$recipient" --rpc-url "$RPC" --unlocked --from "$next" >/dev/null

node --input-type=module - "$chain" "$previous" "$next" <<'JS'
import fs from 'node:fs';
import assert from 'node:assert/strict';
const [chain, previous, next] = process.argv.slice(2).map(s => s.toLowerCase());
const load = mode => JSON.parse(fs.readFileSync(`evm/deployments/ownership-${chain}-${mode}.json`, 'utf8'));
const zero = '0x' + '0'.repeat(40);
const initial = load('initial'), pending = load('pending'), partial = load('partial'), accepted = load('accepted');
for (const prefix of ['cctp','axelar']) {
  assert.equal(initial[prefix+'Owner'].toLowerCase(), previous);
  assert.equal(initial[prefix+'PendingOwner'], zero);
  assert.equal(pending[prefix+'Owner'].toLowerCase(), previous);
  assert.equal(pending[prefix+'PendingOwner'].toLowerCase(), next);
  assert.equal(accepted[prefix+'Owner'].toLowerCase(), next);
  assert.equal(accepted[prefix+'PendingOwner'], zero);
  for (const mode of ['nominate','accept','report']) {
    const final = load(mode);
    assert.equal(final[prefix+'Owner'].toLowerCase(), next);
    assert.equal(final[prefix+'PendingOwner'], zero);
  }
  for (const state of [pending, partial, accepted]) {
    assert.equal(state[prefix+'Adapter'], initial[prefix+'Adapter']);
    assert.equal(state[prefix+'Router'], initial.router);
    assert.equal(state.routerAdminRole, initial.routerAdminRole);
    assert.equal(state.routerGuardianRole, initial.routerGuardianRole);
    assert.equal(state.recordedAdminActive, true);
    assert.equal(state.recordedGuardianActive, true);
  }
}
assert.equal(partial.cctpOwner.toLowerCase(), next);
assert.equal(partial.cctpPendingOwner, zero);
assert.equal(partial.axelarOwner.toLowerCase(), previous);
assert.equal(partial.axelarPendingOwner.toLowerCase(), next);
console.log('Actual per-contract reports, pending-owner refusal and repeated handover PASS');
JS

# The ordinary router path continues to work after ownership moves; use the original lane.
token=$(json "$record" token)
treasury=$(json "$record" treasury)
export HYPERION_TOKEN="$token" HYPERION_TREASURY="$treasury"
( cd evm && forge script script/Smoke.s.sol --rpc-url "$RPC" --broadcast --unlocked --sender "$previous" )
echo 'Local multi-contract ownership rehearsal PASS'
