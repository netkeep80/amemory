import { escapeHtml } from "./i386-lab-view.mjs";

function code(value) {
  return `<code>${escapeHtml(value)}</code>`;
}

function semanticRootsHtml(roots, loaded = false) {
  return `<div class="proof-root-list">${roots.map((root) => `
    <div class="proof-root">
      <strong>${escapeHtml(root.role)}</strong>
      ${loaded ? `<span>local handle ${root.localHandle} · substrate-only</span>` : ""}
      ${code(root.source)}
    </div>`).join("")}</div>`;
}

function reactionTableHtml(proof) {
  return `<div class="lab-table-wrap"><table class="lab-table proof-reaction-table">
    <thead>
      <tr>
        <th>step</th>
        <th>A-memory</th>
        <th>Scope before</th>
        <th>matches</th>
        <th>transitioned</th>
        <th>handoff</th>
        <th>Scope after</th>
        <th>Links</th>
        <th>Q</th>
      </tr>
    </thead>
    <tbody>
      ${proof.execute.reactions.map((step) => `<tr>
        <td>${step.step}</td>
        <td><code>${escapeHtml(step.memoryInstanceId)}</code></td>
        <td>${step.scopeBefore.map(code).join("<br>")}</td>
        <td>${step.rawRuleMatches}</td>
        <td>${step.transitionedMembers}</td>
        <td>${step.handoffCount}</td>
        <td>${step.scopeAfter.map(code).join("<br>")}</td>
        <td>${step.linksAfter}</td>
        <td>${step.quiescent ? "YES" : "—"}</td>
      </tr>`).join("")}
    </tbody>
  </table></div>`;
}

function visualNetworkFromProof(proof) {
  return {
    links: proof.result.visualLinks.map((link) => ({
      key: link.key,
      startKey: link.startKey,
      endKey: link.endKey,
      ...(link.label ? { label: link.label } : {}),
      ...(link.tags?.length ? { tags: link.tags } : {}),
    })),
  };
}

export function proofPipelineHtml(proof) {
  const id = proof.load.memoryInstanceId;
  const ids = new Set([
    proof.load.memoryInstanceId,
    proof.execute.memoryInstanceId,
    proof.result.memoryInstanceId,
    ...proof.execute.reactions.map((step) => step.memoryInstanceId),
  ]);
  const oneMemory = ids.size === 1;
  const asetText = proof.prepare.asetAnums
    .map((source, index) => `${String(index + 1).padStart(4, "0")}  ${source}`)
    .join("\n");

  return `
    <section class="proof-pipeline" data-memory-instance="${escapeHtml(id)}">
      <div class="proof-title">
        <div>
          <h3>Structural proof: portable Aset → one A-memory → result</h3>
          <p>This evidence comes from the same WASM execution that produced the visible MUX1 result.</p>
        </div>
        <div class="proof-memory ${oneMemory ? "proof-pass" : "proof-fail"}">
          <small>ONE RUNTIME A-MEMORY</small>
          <strong>${escapeHtml(id)}</strong>
          <span>${oneMemory ? "same identity in load, every reaction and result" : "IDENTITY MISMATCH"}</span>
        </div>
      </div>

      <div class="proof-stages">
        <article class="proof-stage proof-prepare">
          <div class="proof-stage-number">1</div>
          <div>
            <h4>PREPARE ASET</h4>
            <p><strong>No runtime A-memory exists yet.</strong> ${escapeHtml(proof.prepare.compilerLabel)}</p>
            <div class="proof-kpis">
              <span><small>Portable Anums</small><strong>${proof.prepare.asetAnums.length}</strong></span>
              <span><small>Runtime memory</small><strong>${proof.prepare.runtimeMemoryExists ? "CREATED" : "NOT CREATED"}</strong></span>
            </div>
            <h5>Semantic roots inside the portable image</h5>
            ${semanticRootsHtml(proof.prepare.semanticRoots)}
            <details>
              <summary>Complete portable Aset (${proof.prepare.asetAnums.length} Anums)</summary>
              <pre class="proof-aset">${escapeHtml(asetText)}</pre>
            </details>
          </div>
        </article>

        <div class="proof-arrow">↓ portable Anums only</div>

        <article class="proof-stage proof-load">
          <div class="proof-stage-number">2</div>
          <div>
            <h4>LOAD INTO A-MEMORY</h4>
            <p>One fresh runtime Link store is created here and keeps identity <code>${escapeHtml(id)}</code> through all remaining stages.</p>
            <div class="proof-kpis">
              <span><small>Links before load</small><strong>${proof.load.linksBeforeLoad}</strong></span>
              <span><small>Links after load</small><strong>${proof.load.linksAfterLoad}</strong></span>
              <span><small>Imported Anums</small><strong>${proof.load.importedAnums}</strong></span>
              <span><small>Portable round-trip</small><strong>${proof.load.portableRoundTrip ? "PASS" : "FAIL"}</strong></span>
            </div>
            <h5>Roots resolved in this same memory</h5>
            ${semanticRootsHtml(proof.load.semanticRoots, true)}
          </div>
        </article>

        <div class="proof-arrow">↓ same ${escapeHtml(id)}</div>

        <article class="proof-stage proof-execute">
          <div class="proof-stage-number">3</div>
          <div>
            <h4>EXECUTE IN THE SAME A-MEMORY</h4>
            <p>No reload and no replacement store. The structural interpreter, Theory, function, data and Scope are Links inside <code>${escapeHtml(id)}</code>.</p>
            <div class="proof-kpis">
              <span><small>Active reactions</small><strong>${proof.execute.activeReactionCount}</strong></span>
              <span><small>Final quiescence</small><strong>${proof.execute.finalQuiescent ? "PASS" : "FAIL"}</strong></span>
            </div>
            ${reactionTableHtml(proof)}
          </div>
        </article>

        <div class="proof-arrow">↓ same ${escapeHtml(id)}</div>

        <article class="proof-stage proof-result">
          <div class="proof-stage-number">4</div>
          <div>
            <h4>RESULT + VISUALIZATION</h4>
            <p>The result is exported from the same runtime store. The host comparison below is an independent oracle only.</p>
            <div class="proof-kpis">
              <span><small>Decoded result</small><strong>${proof.result.decodedValue}</strong></span>
              <span><small>Host oracle</small><strong>${proof.result.oracleValue}</strong></span>
              <span><small>Oracle comparison</small><strong>${proof.result.oracleMatches ? "PASS" : "FAIL"}</strong></span>
              <span><small>Final Links</small><strong>${proof.result.linksFinal}</strong></span>
              <span><small>Identical rerun ΔLinks</small><strong>${proof.result.identicalRerunLinkDelta}</strong></span>
            </div>
            <div class="proof-result-anums">
              <div><strong>Result Anum</strong>${code(proof.result.resultAnum)}</div>
              <div><strong>Result sequence Anum</strong>${code(proof.result.resultSequenceAnum)}</div>
            </div>
            <div class="proof-visual-toolbar">
              <div>
                <strong>Same-memory Link topology</strong>
                <span>${proof.result.visualLinks.length} runtime Links · projected directly from ${escapeHtml(id)}</span>
              </div>
              <div class="proof-visual-tabs" data-proof-visual-tabs>
                <button type="button" data-proof-view="blueprint" aria-pressed="true">Blueprint 2D</button>
                <button type="button" data-proof-view="static3d" aria-pressed="false">Static 3D</button>
                <button type="button" data-proof-view="live3d" aria-pressed="false">Live physics 3D</button>
              </div>
            </div>
            <div class="proof-visual-note">
              Exact renderer authority: <code>@mts/visual 0.3.0</code> ·
              <code>mts_visual@4b7c8e97fab8</code>. Presentation only; semantic truth remains the runtime A-memory.
            </div>
            <div class="proof-visual" data-proof-visual>
              <div class="notice">Loading exact-pinned mts_visual Blueprint renderer…</div>
            </div>
            <details>
              <summary>Raw VisualLinkNetwork DTO (${proof.result.visualLinks.length} links)</summary>
              <pre class="proof-aset">${escapeHtml(JSON.stringify(visualNetworkFromProof(proof), null, 2))}</pre>
            </details>
          </div>
        </article>
      </div>
    </section>`;
}

const proofVisualCleanup = new WeakMap();

async function destroyProofVisual(target) {
  const cleanup = proofVisualCleanup.get(target);
  if (cleanup) {
    await cleanup();
    proofVisualCleanup.delete(target);
  }
}

async function renderBlueprint(target, network) {
  const mts = await import("./vendor/mts-visual/index.js");
  const scene = mts.buildBlueprintSvgScene(network);
  target.classList.remove("proof-visual-three");
  target.innerHTML = mts.serializeBlueprintSvg(scene);
  target.dataset.renderer = "@mts/visual Blueprint 2D";
}

async function renderStatic3D(target, network) {
  const [mts, threeVisual] = await Promise.all([
    import("./vendor/mts-visual/index.js"),
    import("./vendor/mts-visual/three/index.js"),
  ]);
  const initial = mts.createInitialPhysics3DState(network);
  const data = threeVisual.buildVisualThreeSceneData(network, initial);
  target.innerHTML = "";
  target.classList.add("proof-visual-three");
  threeVisual.createVisualThreeRenderer(target, data);
  target.dataset.renderer = "@mts/visual Static 3D";
  proofVisualCleanup.set(target, () => {
    threeVisual.destroyVisualThreeRenderer(target);
  });
}

async function renderLive3D(target, network) {
  const [mts, threeVisual] = await Promise.all([
    import("./vendor/mts-visual/index.js"),
    import("./vendor/mts-visual/three/index.js"),
  ]);
  const initial = mts.createInitialPhysics3DState(network);
  const controller = mts.createLivePhysics3D(network, initial);
  target.innerHTML = "";
  target.classList.add("proof-visual-three");
  threeVisual.createVisualThreeLiveRenderer(target, network, controller);
  target.dataset.renderer = "@mts/visual Live physics 3D";
  proofVisualCleanup.set(target, () => {
    threeVisual.destroyVisualThreeRenderer(target);
  });
}

async function renderProofVisual(target, network, mode) {
  await destroyProofVisual(target);
  target.innerHTML = `<div class="notice">Rendering ${escapeHtml(mode)} from the same runtime Link snapshot…</div>`;
  try {
    if (mode === "blueprint") await renderBlueprint(target, network);
    else if (mode === "static3d") await renderStatic3D(target, network);
    else if (mode === "live3d") await renderLive3D(target, network);
    else throw new Error(`unknown proof visual mode: ${mode}`);
  } catch (error) {
    target.classList.remove("proof-visual-three");
    target.innerHTML = `<div class="notice lab-error">mts_visual rendering failed closed: ${escapeHtml(error.message)}</div>`;
  }
}

export async function renderProofPipeline(target, proof) {
  const previousVisual = target.querySelector?.("[data-proof-visual]");
  if (previousVisual) await destroyProofVisual(previousVisual);

  target.innerHTML = proofPipelineHtml(proof);
  const visual = target.querySelector("[data-proof-visual]");
  const tabs = target.querySelector("[data-proof-visual-tabs]");
  if (!visual || !tabs) return;

  const network = visualNetworkFromProof(proof);
  const activate = async (mode) => {
    tabs.querySelectorAll("[data-proof-view]").forEach((button) => {
      button.setAttribute("aria-pressed", String(button.dataset.proofView === mode));
    });
    await renderProofVisual(visual, network, mode);
  };

  tabs.querySelectorAll("[data-proof-view]").forEach((button) => {
    button.addEventListener("click", () => void activate(button.dataset.proofView));
  });

  await activate("blueprint");
}
