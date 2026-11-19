import { test } from "node:test";
import { spawnSync } from "node:child_process";
import assert from "node:assert/strict";
import { planRestoration, rpcClient, cliCodec } from "../restore-plan.mjs";
const enc = (value) => Buffer.from(JSON.stringify(value)).toString("base64");
const codec = {
  decode: (_, value) => JSON.parse(Buffer.from(value, "base64").toString()),
  encode: (_, value) => enc(value),
};
const key = (id) => ({ contract_data: { durability: "persistent", key: id } });
const keys = [1, 2, 3, 4].map((id) => enc(key(id)));
const transaction = enc("unsigned recorded invocation");
const fp = (readOnly, readWrite) =>
  enc({
    resources: { footprint: { read_only: readOnly, read_write: readWrite } },
  });
const request = { keys, transaction };
function rpc(sim, entries = [{ key: keys[0], liveUntilLedgerSeq: 150 }]) {
  return async (method, params) => {
    assert.ok(["simulateTransaction", "getLedgerEntries"].includes(method));
    if (method === "simulateTransaction") {
      assert.equal(params.transaction, transaction);
      return { latestLedger: 100, ...sim };
    }
    assert.deepEqual(params.keys, keys);
    return { latestLedger: 100, entries };
  };
}
test("RPC proof separates active, archived, missing and unknown with an exact restore footprint", async () => {
  const result = await planRestoration(
    request,
    rpc({
      transactionData: fp([key(1), key(3)], []),
      restorePreamble: {
        transactionData: fp([], [key(2)]),
        minResourceFee: "9007199254740993",
      },
    }),
    codec,
  );
  assert.deepEqual(
    result.records.map((r) => r.state),
    ["active", "archived", "missing", "unknown"],
  );
  assert.deepEqual(result.restoreOperation.footprint.readWrite, [keys[1]]);
  assert.equal(result.restoreOperation.minResourceFee, "9007199254740993");
  assert.ok(!JSON.stringify(result).includes("recipient"));
});
test("a failed simulation or sequence allocation never proves a missing record was archived", async () => {
  const result = await planRestoration(
    request,
    rpc({
      error: "contract failed",
      transactionData: fp(
        keys.map((_, i) => key(i + 1)),
        [],
      ),
    }),
    codec,
  );
  assert.deepEqual(
    result.records.map((r) => r.state),
    ["active", "unknown", "unknown", "unknown"],
  );
});
test("rejects oversized requests, inexact fees and restoration footprints", async () => {
  for (const minResourceFee of [
    9007199254740993,
    "9223372036854775808",
    "-1",
  ]) {
    await assert.rejects(
      planRestoration(
        request,
        rpc({
          restorePreamble: {
            minResourceFee,
            transactionData: fp([], [key(2)]),
          },
        }),
        codec,
      ),
      /resource fee/,
    );
  }
  await assert.rejects(
    planRestoration(
      { keys: Array(33).fill(keys[0]), transaction },
      rpc({}),
      codec,
    ),
    /1–32/,
  );
  await assert.rejects(
    planRestoration(
      request,
      rpc({
        restorePreamble: {
          minResourceFee: "1",
          transactionData: fp([], Array(33).fill(key(2))),
        },
      }),
      codec,
    ),
    /exceeds 32/,
  );
});
test("refuses temporary keys, malformed XDR, missing TTL evidence and unrequested ledger entries", async () => {
  await assert.rejects(
    planRestoration(
      {
        keys: [enc({ contract_data: { durability: "temporary" } })],
        transaction,
      },
      rpc({}),
      codec,
    ),
    /persistent/,
  );
  await assert.rejects(
    planRestoration({ keys: ["not-base64"], transaction }, rpc({}), codec),
    /base64/,
  );
  await assert.rejects(
    planRestoration(request, rpc({}, [{ key: keys[0] }]), codec),
    /TTL/,
  );
  await assert.rejects(
    planRestoration(
      request,
      rpc({}, [{ key: enc(key(9)), liveUntilLedgerSeq: 150 }]),
      codec,
    ),
    /unrequested/,
  );
});
test("rejects insecure non-local RPC and stale evidence", async () => {
  assert.throws(() => rpcClient("http://example.com"), /HTTPS/);
  const stale = async (method) =>
    method === "simulateTransaction"
      ? { latestLedger: 80 }
      : { latestLedger: 100, entries: [] };
  await assert.rejects(planRestoration(request, stale, codec), /stale/);
});
test(
  "the production Stellar CLI codec roundtrips real Soroban transaction footprints",
  {
    skip: spawnSync("stellar", ["--version"], { timeout: 10000 }).status !== 0,
  },
  () => {
    const data = {
      ext: "v0",
      resources: {
        footprint: { read_only: [], read_write: [] },
        instructions: 0,
        disk_read_bytes: 0,
        write_bytes: 0,
      },
      resource_fee: "0",
    };
    for (const ext of ["v0", { v1: { archived_soroban_entries: [] } }]) {
      data.ext = ext;
      assert.deepEqual(
        cliCodec.decode(
          "SorobanTransactionData",
          cliCodec.encode("SorobanTransactionData", data),
        ),
        data,
      );
    }
  },
);

test("CAP-0066 automatic restoration is proven by bounded read-write indices without fabricating a separate restore operation", async () => {
  const data = enc({
    ext: { v1: { archived_soroban_entries: [1] } },
    resources: {
      footprint: { read_only: [key(3)], read_write: [key(1), key(2)] },
    },
  });
  const result = await planRestoration(
    request,
    rpc({ transactionData: data }),
    codec,
  );
  assert.deepEqual(
    result.records.map((record) => record.state),
    ["active", "archived", "missing", "unknown"],
  );
  assert.equal(result.restoreOperation, null);
  assert.deepEqual(result.automaticRestore.keys, [keys[1]]);
  assert.equal(result.automaticRestore.sorobanTransactionDataXdr, data);
});
test("rejects malformed automatic restoration indices", async () => {
  for (const indices of [[-1], [1], [0.5], [0, 0]]) {
    const data = enc({
      ext: { v1: { archived_soroban_entries: indices } },
      resources: { footprint: { read_only: [], read_write: [key(2)] } },
    });
    await assert.rejects(
      planRestoration(request, rpc({ transactionData: data }), codec),
      /indices/,
    );
  }
});
