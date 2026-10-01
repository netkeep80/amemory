import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const site = path.join(root, "_site");

function requireSha(name) {
  const value = process.env[name] ?? "";
  if (!/^[0-9a-f]{40}$/.test(value)) {
    throw new Error(name + " must be an exact 40-hex Git SHA");
  }
  return value;
}

function readVersion() {
  const version = fs.readFileSync(path.join(root, "VERSION"), "utf8").trim();
  if (!version) throw new Error("VERSION is empty");
  return version;
}

function buildInfo() {
  const mainSha = requireSha("ACCEPTED_SOURCE_SHA");
  const runId = process.env.ACCEPTED_RUN_ID ?? "";
  if (!/^\d+$/.test(runId)) {
    throw new Error("ACCEPTED_RUN_ID must be numeric");
  }

  const info = {
    schemaVersion: 2,
    version: readVersion(),
    mainSha,
    acceptanceRunId: Number(runId),
    builtAt: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
  };

  fs.mkdirSync(site, { recursive: true });
  fs.writeFileSync(
    path.join(site, "build-info.json"),
    JSON.stringify(info, null, 2) + "\n",
  );
  console.log(JSON.stringify(info, null, 2));
}

function replaceRequired(source, from, to, label) {
  if (!source.includes(from)) {
    throw new Error(label + " anchor missing: " + from);
  }
  return source.replace(from, to);
}

function attachCiMatrix() {
  const indexPath = path.join(site, "index.html");
  let html = fs.readFileSync(indexPath, "utf8");
  const script = '  <script src="./ci-matrix.js"></script>\n';
  const marker = "</body>";

  if (!html.includes(script.trim())) {
    if (!html.includes(marker)) {
      throw new Error("index.html has no </body> marker");
    }
    html = html.replace(marker, script + marker);
    fs.writeFileSync(indexPath, html);
  }

  const verified = fs.readFileSync(indexPath, "utf8");
  if (!verified.includes("ci-matrix.js")) {
    throw new Error("CI matrix script was not attached");
  }
  console.log("PAGES_CI_MATRIX_ATTACH=PASS");
}

function cacheBust() {
  const sha = requireSha("ACCEPTED_SOURCE_SHA");

  const indexPath = path.join(site, "index.html");
  let html = fs.readFileSync(indexPath, "utf8");
  html = replaceRequired(
    html,
    'src="./workbench.mjs"',
    'src="./workbench.mjs?v=' + sha + '"',
    "index Workbench",
  );
  fs.writeFileSync(indexPath, html);

  const workbenchPath = path.join(site, "workbench.mjs");
  let workbench = fs.readFileSync(workbenchPath, "utf8");
  for (const module of [
    "scenario-presets.mjs",
    "scenario-worker-client.mjs",
    "proof-view.mjs",
  ]) {
    workbench = replaceRequired(
      workbench,
      'from "./' + module + '"',
      'from "./' + module + '?v=' + sha + '"',
      "Workbench import",
    );
  }
  fs.writeFileSync(workbenchPath, workbench);

  const workerPath = path.join(site, "scenario-worker.mjs");
  let worker = fs.readFileSync(workerPath, "utf8");
  worker = replaceRequired(
    worker,
    'from "./scenario-transport.mjs"',
    'from "./scenario-transport.mjs?v=' + sha + '"',
    "Worker transport import",
  );
  fs.writeFileSync(workerPath, worker);

  const visualLock = JSON.parse(
    fs.readFileSync(path.join(site, "mts-visual-lock.json"), "utf8"),
  );
  const visualCommit = visualLock.commit;
  if (!/^[0-9a-f]{40}$/.test(visualCommit ?? "")) {
    throw new Error("mts_visual lock commit is invalid");
  }

  const proofPath = path.join(site, "proof-view.mjs");
  let proof = fs.readFileSync(proofPath, "utf8");
  proof = replaceRequired(
    proof,
    'import("./vendor/mts-visual-core.bundle.js")',
    'import("./vendor/mts-visual-core.bundle.js?v=' + visualCommit + '")',
    "proof core visual import",
  );
  proof = replaceRequired(
    proof,
    'import("./vendor/mts-visual-three.bundle.js")',
    'import("./vendor/mts-visual-three.bundle.js?v=' + visualCommit + '")',
    "proof 3D visual import",
  );
  fs.writeFileSync(proofPath, proof);

  if (proof.includes("vendor/mts-visual/three/index.js")) {
    throw new Error(
      "cache-busted proof renderer still references unbundled 3D module graph",
    );
  }
  if (!proof.includes("mts-visual-three.bundle.js?v=" + visualCommit)) {
    throw new Error("3D visual bundle cache-bust was not applied");
  }
  if (!workbench.includes("scenario-worker-client.mjs?v=" + sha)) {
    throw new Error("Workbench Worker client cache-bust was not applied");
  }
  if (!workbench.includes("proof-view.mjs?v=" + sha)) {
    throw new Error("Workbench proof-view cache-bust was not applied");
  }
  if (!worker.includes("scenario-transport.mjs?v=" + sha)) {
    throw new Error("Worker transport cache-bust was not applied");
  }

  console.log(
    "PAGES_CACHE_BUST=PASS source=" +
      sha.slice(0, 12) +
      " visual=" +
      visualCommit.slice(0, 12),
  );
}

const command = process.argv[2];
if (command === "build-info") {
  buildInfo();
} else if (command === "attach-ci-matrix") {
  attachCiMatrix();
} else if (command === "cache-bust") {
  cacheBust();
} else {
  throw new Error(
    "usage: node scripts/pages-build.mjs <build-info|attach-ci-matrix|cache-bust>",
  );
}
