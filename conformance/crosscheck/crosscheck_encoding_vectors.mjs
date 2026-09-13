// Cross-check conformance/vectors/encoding.json with an independent RFC 8785
// implementation (npm `canonicalize`, pinned by the caller).
// Usage: node crosscheck_encoding_vectors.mjs <path-to-canonicalize.js> ../vectors/encoding.json
// CI installs canonicalize@5.0.0 into a temporary directory and passes its lib/canonicalize.js.
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const [modulePath, vectorsPath] = process.argv.slice(2);
const canonicalize = (await import(pathToFileURL(modulePath).href)).default;
const vectors = JSON.parse(readFileSync(vectorsPath, "utf8"));
let failures = 0;
const check = (id, value, expectedHex, expectedDigest) => {
  const canon = Buffer.from(canonicalize(value), "utf8");
  const digest = "sha256:" + createHash("sha256").update(canon).digest("hex");
  if (canon.toString("hex") !== expectedHex || digest !== expectedDigest) {
    failures++;
    console.error(`MISMATCH ${id}: ${canon.toString("hex")} ${digest}`);
  } else {
    console.log(`ok ${id}`);
  }
};
for (const v of vectors.canonical) {
  check(v.id, JSON.parse(Buffer.from(v.input_hex, "hex").toString("utf8")), v.canonical_hex, v.digest);
}
const members = ["operation", "subject", "preconditions", "requires", "payload"];
for (const v of vectors.intent) {
  const intent = {};
  for (const m of members) intent[m] = v.envelope[m];
  intent.extensions = Object.fromEntries(
    Object.entries(v.envelope.extensions ?? {}).filter(([k]) => (v.envelope.requires ?? []).includes(k)),
  );
  check(v.id, intent, v.intent_canonical_hex, v.command_digest);
}
process.exit(failures ? 1 : 0);
