function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function code(value) {
  return `<code>${escapeHtml(value)}</code>`;
}

export function recursiveStructureHtml(value) {
  const text = String(value ?? "");
  return `<details class="proof-recursive-structure" data-recursive-length="${text.length}">
    <summary>
      <span>рекурсивная структура</span>
      <small>${text.length} символов · <span class="proof-recursive-action">нажмите, чтобы раскрыть</span></small>
    </summary>
    <code>${escapeHtml(text)}</code>
  </details>`;
}

function hex32(value) {
  return `0x${(value >>> 0).toString(16).padStart(8, "0")}`;
}

function semanticRootsHtml(roots, loaded = false) {
  return `<div class="proof-root-list">${roots.map((root) => `
    <div class="proof-root">
      <strong>${escapeHtml(root.role)}</strong>
      ${loaded ? `<span>локальный дескриптор ${root.localHandle} · только носитель</span>` : ""}
      ${recursiveStructureHtml(root.source)}
    </div>`).join("")}</div>`;
}

function reactionTableHtml(proof) {
  return `<div class="lab-table-wrap"><table class="lab-table proof-reaction-table">
    <thead>
      <tr>
        <th>шаг</th>
        <th>апамять</th>
        <th>область до</th>
        <th>совпадения</th>
        <th>переходы</th>
        <th>передачи</th>
        <th>область после</th>
        <th>связи Links</th>
        <th>покой</th>
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
        <td>${step.quiescent ? "ДА" : "—"}</td>
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
  const wideResult =
    Number.isInteger(proof.result.decodedValueHi) &&
    Number.isInteger(proof.result.oracleValueHi);
  const decodedResult = wideResult
    ? `${hex32(proof.result.decodedValueHi)}:${hex32(proof.result.decodedValue)}`
    : String(proof.result.decodedValue);
  const oracleResult = wideResult
    ? `${hex32(proof.result.oracleValueHi)}:${hex32(proof.result.oracleValue)}`
    : String(proof.result.oracleValue);
  const resultRecursiveWire =
    proof.result.resultRecursiveWire ?? proof.result.resultAnum;
  const carrierText = proof.prepare.carrierDuplets
    .map((duplet, index) =>
      `${String(index + 1).padStart(6, "0")}  (${duplet.start}, ${duplet.end})`
    )
    .join("\n");

  return `
    <section class="proof-pipeline" data-memory-instance="${escapeHtml(id)}">
      <div class="proof-title">
        <div>
          <h3>Структурное доказательство: упакованная асеть дуплетов → одна апамять → результат</h3>
          <p>Эти доказательства получены тем же исполнением WASM, которое сформировало видимый результат блока.</p>
        </div>
        <div class="proof-memory ${oneMemory ? "proof-pass" : "proof-fail"}">
          <small>ОДНА ИСПОЛНЯЕМАЯ АПАМЯТЬ</small>
          <strong>${escapeHtml(id)}</strong>
          <span>${oneMemory ? "одна и та же идентичность при загрузке, во всех реакциях и в результате" : "НЕСОВПАДЕНИЕ ИДЕНТИЧНОСТИ"}</span>
        </div>
      </div>

      <div class="proof-stages">
        <article class="proof-stage proof-prepare">
          <div class="proof-stage-number">1</div>
          <div>
            <h4>ПОДГОТОВКА АСЕТИ</h4>
            <p><strong>Исполняемая апамять ещё не создана.</strong> Упакованная структурная асеть подготовлена до создания рабочего хранилища.</p>
            <div class="proof-kpis">
              <span><small>Скомпилировано связей Links</small><strong>${proof.prepare.compiledLinks}</strong></span>
              <span><small>Упаковано дуплетов</small><strong>${proof.prepare.carrierDuplets.length}</strong></span>
              <span><small>Исполняемая память</small><strong>${proof.prepare.runtimeMemoryExists ? "СОЗДАНА" : "НЕ СОЗДАНА"}</strong></span>
            </div>
            <h5>Семантические корни внутри упакованного образа</h5>
            ${semanticRootsHtml(proof.prepare.semanticRoots)}
            <details>
              <summary>Структурные правила, допущенные в теорию (${proof.prepare.theoryAdmissions?.length || 0})</summary>
              <pre class="proof-aset">${escapeHtml((proof.prepare.theoryAdmissions || []).join("\n"))}</pre>
            </details>
            <details>
              <summary>Полная упакованная асеть (${proof.prepare.carrierDuplets.length} дуплетов)</summary>
              <pre class="proof-aset">${escapeHtml(carrierText)}</pre>
            </details>
          </div>
        </article>

        <div class="proof-arrow">↓ упакованный носитель (startIndex, endIndex)</div>

        <article class="proof-stage proof-load">
          <div class="proof-stage-number">2</div>
          <div>
            <h4>ЗАГРУЗКА В АПАМЯТЬ</h4>
            <p>Здесь создаётся одно новое рабочее хранилище связей Link и сохраняет идентичность <code>${escapeHtml(id)}</code> на всех последующих стадиях.</p>
            <div class="proof-kpis">
              <span><small>Связей до загрузки</small><strong>${proof.load.linksBeforeLoad}</strong></span>
              <span><small>Связей после загрузки</small><strong>${proof.load.linksAfterLoad}</strong></span>
              <span><small>Полный образ</small><strong>${proof.load.linksAfterLoad === proof.prepare.compiledLinks ? "ПРОЙДЕНО" : "НЕ ПРОЙДЕНО"}</strong></span>
              <span><small>Импортировано дуплетов</small><strong>${proof.load.importedDuplets}</strong></span>
              <span><small>Проверка носителя</small><strong>${proof.load.carrierRoundTrip ? "ПРОЙДЕНО" : "НЕ ПРОЙДЕНО"}</strong></span>
            </div>
            <h5>Корни, разрешённые в этой же памяти</h5>
            ${semanticRootsHtml(proof.load.semanticRoots, true)}
          </div>
        </article>

        <div class="proof-arrow">↓ та же апамять ${escapeHtml(id)}</div>

        <article class="proof-stage proof-execute">
          <div class="proof-stage-number">3</div>
          <div>
            <h4>ИСПОЛНЕНИЕ В ТОЙ ЖЕ АПАМЯТИ</h4>
            <p>Без повторной загрузки и без замены хранилища. Структурный интерпретатор, теория, функция, данные и область Scope представлены связями Links внутри <code>${escapeHtml(id)}</code>.</p>
            <div class="proof-kpis">
              <span><small>Активных реакций</small><strong>${proof.execute.activeReactionCount}</strong></span>
              <span><small>Итоговый покой</small><strong>${proof.execute.finalQuiescent ? "ПРОЙДЕНО" : "НЕ ПРОЙДЕНО"}</strong></span>
            </div>
            ${reactionTableHtml(proof)}
          </div>
        </article>

        <div class="proof-arrow">↓ та же апамять ${escapeHtml(id)}</div>

        <article class="proof-stage proof-result">
          <div class="proof-stage-number">4</div>
          <div>
            <h4>РЕЗУЛЬТАТ И ВИЗУАЛИЗАЦИЯ</h4>
            <p>Результат экспортируется из того же рабочего хранилища. Сравнение на стороне хоста ниже — только независимый эталон.</p>
            <div class="proof-kpis">
              <span><small>Декодировано ${wideResult ? "старшая:младшая" : "значение"}</small><strong>${decodedResult}</strong></span>
              <span><small>Эталон хоста ${wideResult ? "старшая:младшая" : ""}</small><strong>${oracleResult}</strong></span>
              <span><small>Сравнение с эталоном</small><strong>${proof.result.oracleMatches ? "ПРОЙДЕНО" : "НЕ ПРОЙДЕНО"}</strong></span>
              <span><small>Итоговых связей Links</small><strong>${proof.result.linksFinal}</strong></span>
              <span><small>Повтор того же запуска ΔLinks</small><strong>${proof.result.identicalRerunLinkDelta}</strong></span>
            </div>
            <div class="proof-result-anums">
              <div><strong>Рекурсивная запись связи результата</strong>${recursiveStructureHtml(resultRecursiveWire)}</div>
              <div><strong>Последовательностное ачисло результата</strong>${code(proof.result.resultSequenceAnum)}</div>
            </div>
            <div class="proof-visual-toolbar">
              <div>
                <strong>Топология связей той же апамяти</strong>
                <span>${proof.result.visualLinks.length} рабочих связей Links · проекция прямо из ${escapeHtml(id)}</span>
              </div>
              <div class="proof-visual-tabs" data-proof-visual-tabs>
                <button type="button" data-proof-view="blueprint" aria-pressed="true">Схема 2D</button>
                <button type="button" data-proof-view="static3d" aria-pressed="false">Статическая 3D</button>
                <button type="button" data-proof-view="live3d" aria-pressed="false">Живая физика 3D</button>
              </div>
            </div>
            <div class="proof-visual-note">
              Зафиксированный визуализатор: <code>@mts/visual 0.5.0</code> ·
              <code>mts_visual@ccb23ffacb6e</code>. Живая физика использует совместимый постоянный пакетный путь Octahedral Link3D из принятого пакета 0.5.0. Это только представление; семантическим источником истины остаётся исполняемая апамять.
            </div>
            <div class="proof-visual" data-proof-visual>
              <div class="notice">Загрузка точно зафиксированного визуализатора схемы mts_visual…</div>
            </div>
            <details>
              <summary>Сырой DTO VisualLinkNetwork (${proof.result.visualLinks.length} связей)</summary>
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
  const mts = await import("./vendor/mts-visual-core.bundle.js");
  const scene = mts.buildBlueprintSvgScene(network);
  target.classList.remove("proof-visual-three");
  target.innerHTML = mts.serializeBlueprintSvg(scene);
  target.dataset.renderer = "@mts/visual · Схема 2D";
}

async function renderStatic3D(target, network) {
  const [mts, threeVisual] = await Promise.all([
    import("./vendor/mts-visual-core.bundle.js"),
    import("./vendor/mts-visual-three.bundle.js"),
  ]);
  const initial = mts.createInitialPhysics3DState(network);
  const data = threeVisual.buildVisualThreeSceneData(network, initial);
  target.innerHTML = "";
  target.classList.add("proof-visual-three");
  threeVisual.createVisualThreeRenderer(target, data);
  target.dataset.renderer = "@mts/visual · Статическая 3D";
  proofVisualCleanup.set(target, () => {
    threeVisual.destroyVisualThreeRenderer(target);
  });
}

async function renderLive3D(target, network) {
  const [mts, threeVisual] = await Promise.all([
    import("./vendor/mts-visual-core.bundle.js"),
    import("./vendor/mts-visual-three.bundle.js"),
  ]);
  const controller = mts.createOctahedralLivePhysics3D(network, {
    aspectRatio: 2 * Math.SQRT2,
    stiffness: 1,
    simulationSpeed: 1,
  });
  target.innerHTML = "";
  target.classList.add("proof-visual-three");
  const snapshot = threeVisual.createOctahedralThreeLiveRenderer(target, controller);
  target.dataset.renderer = "@mts/visual · Живая физика 3D";
  target.dataset.drawCallProxy = String(snapshot.drawCallProxy);
  proofVisualCleanup.set(target, () => {
    threeVisual.destroyOctahedralThreeLiveRenderer(target);
  });
}

async function renderProofVisual(target, network, mode) {
  await destroyProofVisual(target);
  target.innerHTML = `<div class="notice">Построение ${escapeHtml(mode)} по тому же снимку рабочих связей Link…</div>`;
  try {
    if (mode === "blueprint") await renderBlueprint(target, network);
    else if (mode === "static3d") await renderStatic3D(target, network);
    else if (mode === "live3d") await renderLive3D(target, network);
    else throw new Error(`неизвестный режим визуализации доказательства: ${mode}`);
  } catch (error) {
    target.classList.remove("proof-visual-three");
    target.innerHTML = `<div class="notice lab-error">визуализация mts_visual завершилась с ошибкой: ${escapeHtml(error.message)}</div>`;
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
