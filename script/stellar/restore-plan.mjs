#!/usr/bin/env node
// Read-only RPC planner. Simulation proves archival via preamble or CAP-0066 indices;
// absence from getLedgerEntries alone is never evidence of either archival or deletion.
import { execFileSync } from "node:child_process";
import { openSync, readSync, closeSync } from "node:fs";
import { pathToFileURL } from "node:url";

const LIMIT = 32;
const MAX_XDR = 262144;
function xdrText(value, limit = MAX_XDR) {
  if (
    typeof value !== "string" ||
    !value.length ||
    value.length > limit ||
    !/^[A-Za-z0-9+/]*={0,2}$/.test(value) ||
    Buffer.from(value, "base64").toString("base64") !== value
  ) {
    throw new Error("Expected bounded canonical base64 XDR");
  }
  return value;
}
export const cliCodec = {
  decode(type, value) {
    return JSON.parse(
      execFileSync(
        "stellar",
        ["xdr", "decode", "--type", type, "--input", "single-base64"],
        {
          input: xdrText(value),
          encoding: "utf8",
          timeout: 10000,
          maxBuffer: 1048576,
        },
      ),
    );
  },
  encode(type, value) {
    return xdrText(
      execFileSync("stellar", ["xdr", "encode", "--type", type], {
        input: JSON.stringify(value),
        encoding: "utf8",
        timeout: 10000,
        maxBuffer: 1048576,
      }).trim(),
    );
  },
};
function footprint(data, codec) {
  const decoded = codec.decode("SorobanTransactionData", data);
  const value = decoded?.resources?.footprint;
  if (!Array.isArray(value?.read_only) || !Array.isArray(value?.read_write))
    throw new Error("Invalid simulation footprint");
  if (value.read_only.length + value.read_write.length > LIMIT)
    throw new Error(
      "Simulation footprint exceeds 32 keys; split the requested operation",
    );
  const readOnly = value.read_only.map((key) => codec.encode("LedgerKey", key));
  const readWrite = value.read_write.map((key) =>
    codec.encode("LedgerKey", key),
  );
  const indices = decoded?.ext?.v1?.archived_soroban_entries ?? [];
  if (
    !Array.isArray(indices) ||
    indices.length > readWrite.length ||
    indices.some(
      (index) =>
        !Number.isInteger(index) || index < 0 || index >= readWrite.length,
    )
  ) {
    throw new Error("Invalid automatic restoration indices");
  }
  return {
    readOnly,
    readWrite,
    automaticRestoreKeys: [
      ...new Set(indices.map((index) => readWrite[index])),
    ],
  };
}
export async function planRestoration(request, rpc, codec = cliCodec) {
  if (
    !Array.isArray(request?.keys) ||
    !request.keys.length ||
    request.keys.length > LIMIT
  )
    throw new Error("Supply 1–32 recorded ledger keys");
  const keys = [...new Set(request.keys.map((key) => xdrText(key, 4096)))];
  for (const key of keys) {
    const decoded = codec.decode("LedgerKey", key);
    if (
      !decoded?.contract_code &&
      decoded?.contract_data?.durability !== "persistent"
    )
      throw new Error("Only persistent contract data and code can be restored");
  }
  const transaction = xdrText(request.transaction);
  const simulation = await rpc("simulateTransaction", { transaction });
  const preamble = simulation.restorePreamble;
  if (
    preamble &&
    (typeof preamble.minResourceFee !== "string" ||
      !/^[0-9]{1,19}$/.test(preamble.minResourceFee) ||
      BigInt(preamble.minResourceFee) > 9223372036854775807n)
  )
    throw new Error("Invalid restoration resource fee");
  const restore = preamble
    ? footprint(xdrText(preamble.transactionData), codec)
    : { readOnly: [], readWrite: [] };
  if (restore.readOnly.length)
    throw new Error("Restore preamble must contain only read-write keys");
  const invoked = simulation.transactionData
    ? footprint(xdrText(simulation.transactionData), codec)
    : { readOnly: [], readWrite: [], automaticRestoreKeys: [] };
  const restoreKeys = new Set([
    ...restore.readWrite,
    ...invoked.automaticRestoreKeys,
  ]);
  const touched = new Set([...invoked.readOnly, ...invoked.readWrite]);
  const ledger = await rpc("getLedgerEntries", { keys });
  if (
    !Number.isSafeInteger(ledger.latestLedger) ||
    ledger.latestLedger < 1 ||
    !Number.isSafeInteger(simulation.latestLedger) ||
    Math.abs(ledger.latestLedger - simulation.latestLedger) > 8
  )
    throw new Error("Invalid or stale RPC ledger evidence; simulate again");
  if (!Array.isArray(ledger.entries) || ledger.entries.length > keys.length)
    throw new Error("Invalid ledger response");
  const active = new Set();
  for (const entry of ledger.entries) {
    if (!keys.includes(entry.key))
      throw new Error("RPC returned an unrequested key");
    if (!Number.isSafeInteger(entry.liveUntilLedgerSeq))
      throw new Error("RPC omitted persistent TTL evidence");
    if (entry.liveUntilLedgerSeq >= ledger.latestLedger) active.add(entry.key);
  }
  const records = keys.map((key) => {
    if (restoreKeys.has(key)) return { key, state: "archived" };
    if (active.has(key)) return { key, state: "active" };
    if (!simulation.error && touched.has(key)) return { key, state: "missing" };
    return {
      key,
      state: "unknown",
      reason:
        "RPC absence without a successful matching footprint is inconclusive",
    };
  });
  return {
    version: 1,
    latestLedger: ledger.latestLedger,
    simulationLedger: simulation.latestLedger,
    records,
    restoreOperation: preamble
      ? {
          type: "restoreFootprint",
          footprint: restore,
          sorobanTransactionDataXdr: preamble.transactionData,
          minResourceFee: String(preamble.minResourceFee),
        }
      : null,
    automaticRestore: invoked.automaticRestoreKeys.length
      ? {
          keys: invoked.automaticRestoreKeys,
          sorobanTransactionDataXdr: simulation.transactionData,
        }
      : null,
    nextStep: preamble
      ? "Review and restore exactly this footprint, then re-simulate the original operation"
      : invoked.automaticRestoreKeys.length
        ? "Review automatic restoration keys and transaction data, then re-simulate before signing"
        : "Review record states and re-simulate before signing",
    simulationError: simulation.error ?? null,
  };
}
export function rpcClient(url) {
  const endpoint = new URL(url);
  if (
    endpoint.protocol !== "https:" &&
    !(
      endpoint.protocol === "http:" &&
      ["127.0.0.1", "localhost", "[::1]"].includes(endpoint.hostname)
    )
  )
    throw new Error("Use HTTPS or a local RPC endpoint");
  return async (method, params) => {
    const response = await fetch(endpoint, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }),
      signal: AbortSignal.timeout(20000),
      redirect: "error",
    });
    if (!response.ok) throw new Error(`RPC HTTP ${response.status}`);
    if (!response.body) throw new Error("RPC response has no body");
    const reader = response.body.getReader();
    const chunks = [];
    let length = 0;
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      length += value.byteLength;
      if (length > 1048576) {
        await reader.cancel();
        throw new Error("RPC response exceeds 1MiB");
      }
      chunks.push(Buffer.from(value));
    }
    const data = JSON.parse(Buffer.concat(chunks, length).toString("utf8"));
    if (data.error || !data.result) throw new Error(`RPC ${method} failed`);
    return data.result;
  };
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  try {
    const [url, file, ...extra] = process.argv.slice(2);
    if (!url || !file || extra.length)
      throw new Error(
        "Usage: node script/stellar/restore-plan.mjs RPC_URL recorded-request.json",
      );
    const descriptor = openSync(file, "r");
    const buffer = Buffer.alloc(MAX_XDR * 2 + 1);
    let length = 0;
    try {
      while (length < buffer.length) {
        const count = readSync(
          descriptor,
          buffer,
          length,
          buffer.length - length,
          null,
        );
        if (count === 0) break;
        length += count;
      }
    } finally {
      closeSync(descriptor);
    }
    if (length > MAX_XDR * 2) throw new Error("Request file too large");
    const bytes = buffer.subarray(0, length);
    console.log(
      JSON.stringify(
        await planRestoration(
          JSON.parse(bytes.toString("utf8")),
          rpcClient(url),
        ),
        null,
        2,
      ),
    );
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
