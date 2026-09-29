import {
  closeScenarioLiveSession,
  openScenarioLiveSession,
  readScenarioTransportLimits,
  refreshScenarioLiveHistoryStatus,
  runScenarioLiveSession,
  setScenarioLiveRetentionPolicy,
} from "./scenario-transport.mjs";
import {
  loadScenarioPresetManifestByIndex,
  refreshScenarioPresetRegistry,
} from "./scenario-presets.mjs";
import { recursiveStructureHtml } from "./proof-view.mjs";

const BACKENDS = [
  ["optimized-cpu", "CPU · оптимизированный"],
  ["webgpu", "WebGPU"],
  ["linksdb", "LinksDB"],
];

const STAGE_LABELS = Object.freeze({
  PREPARE: "Подготовка",
  LOAD: "Загрузка",
  CONFIGURE: "Настройка",
  EXECUTE: "Исполнение",
  RESULT: "Результат",
  EVIDENCE: "Доказательства",
});
const LEVEL_LABELS = Object.freeze({
  simple: "Простой",
  engineering: "Инженерный",
  proof: "Доказательство",
});
const TAB_LABELS = Object.freeze({
  timeline: "Хронология",
  log: "Журнал",
  profile: "Профиль",
  compare: "Сравнение",
  proof: "Доказательство",
});
const stageLabel = (id) => STAGE_LABELS[id] || id;
const levelLabel = (id) => LEVEL_LABELS[id] || id;
const tabLabel = (id) => TAB_LABELS[id] || id;
const errorText = (error) => error && error.message ? error.message : String(error);

const clone = (value) => JSON.parse(JSON.stringify(value));
const sameInputs = (a, b) => JSON.stringify(a || {}) === JSON.stringify(b || {});

function parseUnsigned(raw, max, label) {
  const text = String(raw).trim();
  const value = /^0x[0-9a-f]+$/i.test(text)
    ? Number.parseInt(text.slice(2), 16)
    : Number.parseInt(text, 10);
  if (!Number.isSafeInteger(value) || value < 0 || value > max) {
    throw new Error(label + ": требуется число от 0 до " + max);
  }
  return value;
}

export function normalizeWorkbenchInputs(schema, raw) {
  const out = {};
  for (const field of schema || []) {
    const type = String(field.type || "").toUpperCase();
    const value = raw ? raw[field.key] : undefined;
    if (type === "BIT") {
      const bit = Number(value);
      if (bit !== 0 && bit !== 1) throw new Error(field.key + ": допустимы только 0 или 1");
      out[field.key] = bit;
    } else if (type === "COUNT8" || type === "U8") {
      out[field.key] = parseUnsigned(value, 0xff, field.key);
    } else if (type === "WORD32" || type === "U32" || type === "UINT32") {
      out[field.key] = parseUnsigned(value, 0xffff_ffff, field.key) >>> 0;
    } else if (/^-?\d+$/.test(String(value).trim())) {
      out[field.key] = Number.parseInt(String(value).trim(), 10);
    } else {
      out[field.key] = value;
    }
  }
  return out;
}

export function createWorkbenchRun(manifest, presetIndex, rawInputs, ordinal, mode = "preset") {
  const runs = Array.isArray(manifest && manifest.runSequence) ? manifest.runSequence : [];
  const base = clone(runs[presetIndex] || runs[0] || {
    runId: "workbench-template",
    executionMode: "TO_QUIESCENCE",
    assertions: [],
  });
  const inputs = normalizeWorkbenchInputs(
    (manifest && manifest.inputSchema) || [],
    rawInputs || base.inputs || (manifest && manifest.initialInputs) || {},
  );
  base.runId = "workbench-live-" + ordinal;
  base.inputs = inputs;
  if (mode === "manual") {
    const match = runs.find((run) => sameInputs(run.inputs, inputs));
    base.assertions = match ? clone(match.assertions || []) : [];
  }
  return base;
}

export function deriveWorkbenchPipeline(status, run) {
  const open = Boolean(status);
  const stages = run && run.pipelineProfile && run.pipelineProfile.stages;
  const done = Boolean(run && stages);
  return [
    ["PREPARE", open && status.prepareCount === 1, open ? "prepareCount=" + status.prepareCount : "сессия не открыта"],
    ["LOAD", open && status.loadCount === 1, open ? "базовых связей Links=" + status.baseLinkCount : "сессия не открыта"],
    ["CONFIGURE", done, done ? "связи Links " + run.linksBeforeConfigure + " → " + run.linksAfterConfigure : "запуск ещё не выполнен"],
    ["EXECUTE", done, done ? "реакций=" + (run.observed && run.observed.activeReactionCount) : "запуск ещё не выполнен"],
    ["RESULT", done, done ? "получен реальный результат исполнения" : "запуск ещё не выполнен"],
    ["EVIDENCE", done, done ? "событий=" + ((run.observed && run.observed.events || []).length) : "запуск ещё не выполнен"],
  ].map(([id, complete, detail]) => ({ id, state: complete ? "done" : "waiting", detail }));
}


export function workbenchResultStatus(run) {
  if (!run) {
    return {
      kind: "ready",
      label: "ГОТОВО",
      explanation: "Программа загружена один раз. Нажмите «Выполнить», чтобы запустить её в этой же сессии апамяти.",
    };
  }
  const assertions = Array.isArray(run.assertionResults)
    ? run.assertionResults
    : [];
  if (assertions.length === 0) {
    return {
      kind: "neutral",
      label: "РЕЗУЛЬТАТ",
      explanation: "Получен реальный результат исполнения. Для этого ручного набора входов нет эталонной проверки, поэтому интерфейс не придумывает статус «пройдено/не пройдено».",
    };
  }
  const passed = assertions.every((item) => item.passed === true);
  return {
    kind: passed ? "pass" : "fail",
    label: passed ? "ПРОЙДЕНО" : "НЕ ПРОЙДЕНО",
    explanation: passed
      ? "Результат исполнения удовлетворяет всем проверкам, заданным этим готовым сценарием."
      : "Хотя бы одна проверка готового сценария не совпала с реальным результатом исполнения.",
  };
}

export function workbenchStageDetails(status, run, stageId) {
  if (stageId === "PREPARE") {
    return status ? {
      sessionId: status.sessionId,
      programProfile: status.programProfile,
      programFingerprint: status.programFingerprint,
      prepareCount: status.prepareCount,
      profile: status.sessionOpenProfile?.stages ?? null,
    } : null;
  }
  if (stageId === "LOAD") {
    return status ? {
      sessionId: status.sessionId,
      loadCount: status.loadCount,
      baseLinkCount: status.baseLinkCount,
      currentLinkCount: status.currentLinkCount,
      profile: status.sessionOpenProfile?.stages ?? null,
    } : null;
  }
  if (!run) return null;
  if (stageId === "CONFIGURE") {
    return {
      sessionRunId: run.sessionRunId,
      configurationReused: run.configurationReused,
      linksBeforeConfigure: run.linksBeforeConfigure,
      linksAfterConfigure: run.linksAfterConfigure,
      profile: run.pipelineProfile?.stages ?? null,
    };
  }
  if (stageId === "EXECUTE") {
    return {
      sessionRunId: run.sessionRunId,
      finalScope: run.observed?.finalScope ?? [],
      activeReactionCount: run.observed?.activeReactionCount ?? null,
      finalQuiescent: run.observed?.finalQuiescent ?? null,
      executeProfile: run.observed?.profile ?? null,
    };
  }
  if (stageId === "RESULT") {
    return {
      sessionRunId: run.sessionRunId,
      result: run.result,
      assertionResults: run.assertionResults ?? [],
      oracleMatches: run.oracleMatches ?? null,
    };
  }
  if (stageId === "EVIDENCE") {
    return {
      sessionRunId: run.sessionRunId,
      eventCount: run.observed?.events?.length ?? 0,
      observationLevel: run.observed?.observationLevel ?? null,
      pipelineProfile: run.pipelineProfile ?? null,
    };
  }
  return null;
}

const esc = (value) => String(value == null ? "" : value)
  .replaceAll("&", "&amp;").replaceAll("<", "&lt;")
  .replaceAll(">", "&gt;").replaceAll('"', "&quot;")
  .replaceAll("'", "&#39;");
const json = (value) => esc(JSON.stringify(value, null, 2));
const compact = (value) => {
  const text = String(value == null ? "—" : value);
  return text.length <= 22 ? text : text.slice(0, 10) + "…" + text.slice(-9);
};

function timing(stages, key) {
  if (!stages || stages.timingAvailable === false) return "время недоступно";
  return Number.isFinite(stages[key]) ? stages[key] + " ns" : "нет данных";
}

function resultText(run) {
  if (!run) return "Результата ещё нет";
  const result = run.result;
  if (result == null || typeof result !== "object") return String(result);
  if ("valueHi" in result && "value" in result) return "старшая=" + result.valueHi + " · младшая=" + result.value;
  return "value" in result ? String(result.value) : JSON.stringify(result);
}

function assertionText(run) {
  const items = Array.isArray(run && run.assertionResults) ? run.assertionResults : [];
  if (!items.length) return "для этого ручного ввода нет эталонной проверки";
  return items.filter((item) => item.passed === true).length + "/" + items.length + " проверок пройдено";
}

function styles() {
  if (document.querySelector("#amemory-workbench-style")) return;
  const style = document.createElement("style");
  style.id = "amemory-workbench-style";
  style.textContent = [
    ".wb{margin:18px 0 30px;display:grid;gap:12px}",
    ".wb-card{background:var(--surface);border:1px solid var(--line);border-radius:16px;box-shadow:var(--shadow);min-width:0}",
    ".wb-top{padding:14px 16px;display:flex;gap:8px;flex-wrap:wrap;align-items:center}.wb-top strong{margin-right:auto}",
    ".wb-chip{padding:5px 8px;border:1px solid var(--line);border-radius:999px;background:var(--surface-2);font-size:.76rem}.wb-chip.good{border-color:var(--good);color:var(--good)}.wb-chip.bad{border-color:var(--bad);color:var(--bad)}",
    ".wb-pipe{display:grid;grid-template-columns:repeat(6,minmax(0,1fr));gap:8px}.wb-stage{padding:10px;border:1px solid var(--line);border-radius:12px;background:var(--surface);color:var(--text);text-align:left;cursor:pointer}.wb-stage.done{border-color:var(--good)}.wb-stage strong{display:block;font-size:.76rem}.wb-stage small{display:block;color:var(--muted);margin-top:4px;overflow-wrap:anywhere}",
    ".wb-grid{display:grid;grid-template-columns:minmax(300px,360px) minmax(0,1fr);gap:12px;align-items:start}.wb-controls,.wb-main{padding:16px}.wb-controls h3,.wb-main h3{margin:0 0 12px}",
    ".wb-field{display:grid;gap:5px;margin:10px 0}.wb-field label{color:var(--muted);font-size:.78rem;font-weight:700}.wb-field input,.wb-field select,.wb-actions button,.wb-mode button{min-height:38px;border:1px solid var(--line);border-radius:9px;background:var(--surface-2);color:var(--text);padding:7px 9px}.wb-field input,.wb-field select{width:100%}",
    ".wb-mode,.wb-actions{display:flex;gap:7px;flex-wrap:wrap}.wb-mode button,.wb-actions button{cursor:pointer}.wb-mode button[aria-pressed=true]{outline:2px solid var(--accent);font-weight:800}.wb-actions .primary{background:var(--accent);color:#fff;border-color:var(--accent);font-weight:800}.wb-actions button:disabled{opacity:.45}",
    ".wb-help{color:var(--muted);font-size:.78rem;margin-top:6px}.wb-memory{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:8px}.wb-metric{padding:10px;border:1px solid var(--line);border-radius:10px;background:var(--surface-2);min-width:0}.wb-metric small{display:block;color:var(--muted)}.wb-metric strong,.wb-metric code{display:block;margin-top:4px;overflow-wrap:anywhere}",
    ".wb-result{margin-top:12px;padding:14px;border:2px solid var(--line);border-radius:12px}.wb-result.ready{border-color:var(--good)}.wb-result h4{margin:0 0 7px}.wb-result-value{font-size:1.35rem;font-weight:900;overflow-wrap:anywhere}",
    ".wb-tabs{padding:0 16px 16px}.wb-tabbar{display:flex;gap:6px;flex-wrap:wrap;border-bottom:1px solid var(--line);padding-bottom:9px}.wb-tabbar button{border:0;background:transparent;color:var(--muted);padding:7px 9px;cursor:pointer}.wb-tabbar button[aria-selected=true]{color:var(--text);font-weight:800;border-bottom:2px solid var(--accent)}.wb-tab{padding-top:12px}",
    ".wb-log{display:grid;gap:6px;max-height:360px;overflow:auto}.wb-log-row{padding:8px 10px;border:1px solid var(--line);border-radius:9px;background:var(--surface-2);font-size:.78rem}.wb-table{width:100%;border-collapse:collapse;font-size:.8rem}.wb-table th,.wb-table td{text-align:left;padding:7px 8px;border-bottom:1px solid var(--line)}.wb-table th{color:var(--muted)}",
    ".wb-raw{margin-top:10px;border:1px solid var(--line);border-radius:10px;overflow:hidden}.wb-raw summary{cursor:pointer;padding:9px 11px;font-weight:700}.wb-raw pre{border-radius:0;max-height:360px;font-size:.75rem}.wb-error{color:var(--bad);font-weight:800}",
    ".wb-proof-structure{margin-top:10px;padding:10px;border:1px solid var(--line);border-radius:10px;background:var(--surface-2)}.proof-recursive-structure{margin-top:8px;border:1px solid var(--line);border-radius:9px;background:var(--surface)}.proof-recursive-structure summary{cursor:pointer;padding:8px 10px}.proof-recursive-structure>code{display:block;padding:10px;max-height:280px;overflow:auto;overflow-wrap:anywhere}",
    ".wb-levels{display:flex;gap:5px;flex-wrap:wrap}.wb-levels button{border:1px solid var(--line);border-radius:999px;background:var(--surface-2);color:var(--muted);padding:5px 8px;cursor:pointer;font-size:.76rem}.wb-levels button[aria-pressed=true]{border-color:var(--accent);color:var(--text);font-weight:800}",
    ".wb-simple-grid{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:8px;margin-bottom:12px}.wb-simple-item{padding:12px;border:1px solid var(--line);border-radius:10px;background:var(--surface-2);min-width:0}.wb-simple-item small{display:block;color:var(--muted)}.wb-simple-item strong,.wb-simple-item code{display:block;margin-top:5px;overflow-wrap:anywhere}.wb-result.pass{border-color:var(--good)}.wb-result.fail{border-color:var(--bad)}",

    "@media(max-width:1000px){.wb-grid{grid-template-columns:1fr}.wb-memory{grid-template-columns:1fr 1fr}.wb-pipe{grid-template-columns:repeat(3,1fr)}}@media(max-width:620px){.wb-memory,.wb-pipe{grid-template-columns:1fr}}",
  ].join("\n");
  document.head.append(style);
}

async function wasm() {
  const response = await fetch("./amemory_a_circuit.wasm", { cache: "no-store" });
  if (!response.ok) throw new Error("Не удалось загрузить A-Circuit WASM: HTTP " + response.status);
  const bytes = await response.arrayBuffer();
  const loaded = await WebAssembly.instantiate(bytes, {});
  const w = loaded.instance.exports;
  for (const name of [
    "amemory_scenario_preset_registry_refresh",
    "amemory_scenario_live_open_json",
    "amemory_scenario_live_execute_json",
    "amemory_scenario_live_close",
  ]) {
    if (typeof w[name] !== "function") throw new Error("В WASM отсутствует функция ABI лаборатории: " + name);
  }
  return w;
}

async function buildInfo() {
  try {
    const response = await fetch("./build-info.json", { cache: "no-store" });
    const info = response.ok ? await response.json() : {};
    return { version: info.version || "неизвестно", sha: info.mainSha || "неизвестно" };
  } catch {
    return { version: "неизвестно", sha: "неизвестно" };
  }
}

function presetInputs(state) {
  const run = state.manifest.runSequence[state.presetIndex];
  state.inputs = clone((run && run.inputs) || state.manifest.initialInputs || {});
}

function loadManifest(state, index) {
  if (state.session) closeScenarioLiveSession(state.wasm);
  const source = loadScenarioPresetManifestByIndex(state.wasm, index);
  if (source == null) throw new Error("Не найден манифест сценария с индексом " + index);
  state.scenarioIndex = index;
  state.manifest = JSON.parse(source);
  state.presetIndex = 0;
  state.mode = "preset";
  state.backend = (state.manifest.supportedИсполнительs || [])[0] || "optimized-cpu";
  state.session = null;
  state.run = null;
  state.history = null;
  state.error = null;
  presetInputs(state);
}

async function history(state) {
  const refreshed = refreshScenarioLiveHistoryStatus(state.wasm);
  state.history = refreshed.ok ? refreshed.status : null;
}

async function open(state) {
  state.loading = true; state.error = null; render(state);
  try {
    const opened = openScenarioLiveSession(state.wasm, state.manifest, state.backend);
    if (!opened.ok) throw new Error(opened.error && opened.error.message || JSON.stringify(opened.error));
    state.session = opened.status;
    const cap = readScenarioTransportLimits(state.wasm).liveObserver;
    const policy = setScenarioLiveRetentionPolicy(state.wasm, {
      retentionMode: "RING",
      maxRetainedRuns: Math.min(8, cap.maxRetainedRuns),
      maxRetainedEvents: Math.min(8192, cap.maxRetainedEvents),
      maxRetainedBytes: Math.min(8 * 1024 * 1024, cap.maxRetainedBytes),
    });
    if (!policy.ok) throw new Error(policy.error && policy.error.message || JSON.stringify(policy.error));
    await history(state);
  } catch (error) {
    state.session = null; state.run = null; state.history = null;
    state.error = "Не удалось открыть апамять: " + errorText(error);
  }
  state.loading = false; render(state);
}

async function execute(state) {
  state.loading = true; state.error = null; render(state);
  try {
    const request = createWorkbenchRun(
      state.manifest, state.presetIndex, state.inputs,
      (state.session.completedRuns || 0) + 1, state.mode,
    );
    const result = runScenarioLiveSession(state.wasm, request);
    if (!result.ok) throw new Error(result.error && result.error.message || JSON.stringify(result.error));
    state.session = result.payload.status;
    state.run = result.payload.run;
    await history(state);
  } catch (error) {
    state.error = "Ошибка исполнения: " + errorText(error);
  }
  state.loading = false; render(state);
}

function close(state) {
  if (state.session) closeScenarioLiveSession(state.wasm);
  state.session = null; state.run = null; state.history = null; state.error = null;
  render(state);
}

export function workbenchRecursiveStructure(run) {
  const result = run && run.result;
  if (!result || typeof result !== "object") return null;
  for (const key of ["resultRecursiveWire", "resultSequenceAnum", "resultAnum", "recursiveWire"]) {
    if (typeof result[key] === "string" && result[key].length > 0) {
      return { key, value: result[key] };
    }
  }
  return null;
}

function tab(state) {
  const run = state.run;
  if (state.tab === "log") {
    const events = run && run.observed && Array.isArray(run.observed.events) ? run.observed.events : [];
    return events.length
      ? '<div class="wb-log">' + events.map((event, i) =>
          '<div class="wb-log-row"><strong>Событие #' + i + " · " + esc(event.kind || "EVENT") +
          '</strong><br><code>' + esc(JSON.stringify(event)) + '</code></div>').join("") + '</div>'
      : '<div class="wb-help">Событий исполнения пока нет.</div>';
  }
  if (state.tab === "profile") {
    const stages = run && run.pipelineProfile && run.pipelineProfile.stages;
    const structural = run && run.observed && run.observed.profile && run.observed.profile.structural;
    if (!stages) return '<div class="wb-help">Выполните хотя бы один запуск, чтобы появился реальный профиль.</div>';
    return '<table class="wb-table"><tbody>' +
      '<tr><th>Настройка</th><td>' + timing(stages, "configureNs") + '</td></tr>' +
      '<tr><th>Исполнение</th><td>' + timing(stages, "executeNs") + '</td></tr>' +
      '<tr><th>Результат</th><td>' + timing(stages, "resultNs") + '</td></tr>' +
      '<tr><th>Доказательства</th><td>' + timing(stages, "evidenceNs") + '</td></tr>' +
      '<tr><th>Кандидаты запуска</th><td>' + esc(structural && structural.triggerIncidenceCandidates || "нет данных") + '</td></tr>' +
      '<tr><th>Попытки унификации</th><td>' + esc(structural && structural.unificationAttempts || "нет данных") + '</td></tr>' +
      '<tr><th>Опубликованные выходы</th><td>' + esc(structural && structural.publicationOutputs || "нет данных") + '</td></tr></tbody></table>';
  }
  if (state.tab === "timeline") {
    const h = state.history;
    const trend = Array.isArray(h?.profileTrend) ? h.profileTrend : [];
    if (!h) return '<div class="wb-help">Хронология появится после открытия сессии.</div>';
    return '<div class="wb-help">Данные наблюдателя · режим ' + esc(h.retentionMode) +
      " · сохранено запусков=" + h.retainedRuns + " · событий=" + h.retainedEvents +
      " · точек профиля=" + h.profilePoints + '</div>' +
      (trend.length === 0
        ? '<div class="wb-help">Завершённых запусков пока нет.</div>'
        : '<table class="wb-table"><thead><tr><th>Запуск</th><th>Исполнитель</th><th>Связей до</th><th>После настройки</th><th>После исполнения</th></tr></thead><tbody>' +
          trend.map((profile) =>
            '<tr><td>' + esc(profile.runId) + '</td><td>' + esc(profile.backendId) +
            '</td><td>' + esc(profile.linksBeforeConfigure) + '</td><td>' +
            esc(profile.linksAfterConfigure) + '</td><td>' +
            esc(profile.linksAfterExecute) + '</td></tr>'
          ).join("") + '</tbody></table>');
  }
  if (state.tab === "compare") {
    const supported = new Set(state.manifest.supportedИсполнительs || []);
    return '<div class="wb-help">Интерфейс не выдумывает дифференциальный результат. Сравнение станет исполняемым, когда один и тот же сценарий будут поддерживать как минимум два постоянных адаптера.</div>' +
      '<table class="wb-table"><thead><tr><th>Исполнитель</th><th>Поддержка сценарием</th><th>Текущее состояние</th></tr></thead><tbody>' +
      BACKENDS.map(([id, label]) =>
        '<tr><td>' + esc(label) + '</td><td>' +
        esc(supported.has(id) ? "заявлена поддержка" : "не заявлена") +
        '</td><td>' + esc(id === state.backend && state.session ? "активная сессия" :
          supported.has(id) ? "доступен после реализации адаптера" : "не поддерживается") +
        '</td></tr>'
      ).join("") + '</tbody></table>' +
      '<div class="wb-help">Постоянные адаптеры WebGPU / LinksDB отслеживаются в #279 / #280; дифференциальное сравнение — в #281.</div>';
  }
  const recursive = workbenchRecursiveStructure(run);
  return '<div class="wb-help">Доказательства и сырые структуры по умолчанию свёрнуты.</div>' +
    (recursive
      ? '<div class="wb-proof-structure"><strong>Переносимая рекурсивная структура · ' + esc(recursive.key) +
        '</strong>' + recursiveStructureHtml(recursive.value) + '</div>'
      : '<div class="wb-help">В нормализованном результате этого запуска нет переносимой рекурсивной структуры.</div>') +
    '<details class="wb-raw"><summary>Полный отчёт текущего запуска</summary><pre>' + json(run || {}) + '</pre></details>' +
    '<details class="wb-raw"><summary>Манифест сценария</summary><pre>' + json(state.manifest || {}) + '</pre></details>';
}
function syncInputs(state) {
  for (const node of state.root.querySelectorAll("[data-input-key]")) {
    state.inputs[node.dataset.inputKey] = node.value;
  }
}

function render(state) {
  const status = state.session;
  const run = state.run;
  const pipeline = deriveWorkbenchPipeline(status, run);
  const openSession = Boolean(status);
  const level = state.level;
  const resultStatus = workbenchResultStatus(run);
  const stageDetail = workbenchStageDetails(status, run, state.stage);
  const currentLinks = status
    ? (status.currentLinkCount == null ? status.baseLinkCount : status.currentLinkCount)
    : "—";

  state.root.innerHTML =
    '<div class="wb">' +
    '<section class="wb-card wb-top"><strong>Рабочая лаборатория апамяти</strong>' +
    '<span class="wb-chip">v' + esc(state.build.version) + '</span>' +
    '<span class="wb-chip" title="' + esc(state.build.sha) + '">SHA ' + esc(compact(state.build.sha)) + '</span>' +
    '<span class="wb-chip">исполнитель ' + esc(state.backend) + '</span>' +
    '<span class="wb-chip">сессия ' + esc(compact(status && status.sessionId)) + '</span>' +
    '<span class="wb-chip">запуск ' + esc(run && run.sessionRunId || 0) + '</span>' +
    '<span class="wb-chip ' + (state.error ? "bad" : openSession ? "good" : "") + '">' +
    esc(state.error ? "ОШИБКА" : openSession ? "СЕССИЯ ОТКРЫТА" : state.loading ? "ЗАГРУЗКА" : "ЗАКРЫТО") +
    '</span><div class="wb-levels" aria-label="Уровень подробности">' +
    ["simple", "engineering", "proof"].map((name) =>
      '<button data-level="' + name + '" aria-pressed="' + (level === name) + '">' +
      esc(levelLabel(name)) + '</button>').join("") +
    '</div></section>' +

    '<section class="wb-pipe">' + pipeline.map((stage) =>
      '<button type="button" data-stage="' + stage.id + '" class="wb-stage ' + stage.state + '"' +
      ' aria-pressed="' + (state.stage === stage.id) + '" title="' + esc(stage.id) + '"><strong>' + esc(stageLabel(stage.id)) +
      '</strong><small>' + esc(stage.detail) + '</small></button>').join("") + '</section>' +

    '<section class="wb-grid"><div class="wb-card wb-controls"><h3>Сценарий / конструктор</h3>' +
    '<div class="wb-field"><label>Готовый сценарий</label><select id="wb-scenario"' +
    (openSession ? " disabled" : "") + '>' +
    (state.registry.entries || []).map((entry, i) =>
      '<option value="' + i + '"' + (i === state.scenarioIndex ? " selected" : "") + '>' +
      esc((entry.title || ("Сценарий " + entry.scenarioId)) +
      (entry.scenarioVersion ? " @" + entry.scenarioVersion : "")) + '</option>').join("") +
    '</select><div class="wb-help">' + esc(state.manifest.description || "") + '</div></div>' +

    '<div class="wb-mode"><button data-mode="preset" aria-pressed="' + (state.mode === "preset") +
    '">Готовый</button><button data-mode="manual" aria-pressed="' + (state.mode === "manual") +
    '">Ручной</button></div>' +

    '<div class="wb-field"><label>Готовый запуск</label><select id="wb-preset-run">' +
    (state.manifest.runSequence || []).map((item, i) =>
      '<option value="' + i + '"' + (i === state.presetIndex ? " selected" : "") + '>' +
      esc("Запуск " + (i + 1) + " · " + (item.runId || ("run-" + (i + 1)))) + '</option>').join("") + '</select></div>' +

    (state.manifest.inputSchema || []).map((field) => {
      const value = state.inputs[field.key];
      const disabled = state.mode === "preset" ? " disabled" : "";
      const control = String(field.type || "").toUpperCase() === "BIT"
        ? '<select data-input-key="' + esc(field.key) + '"' + disabled + '><option value="0"' +
          (Number(value) === 0 ? " selected" : "") + '>0</option><option value="1"' +
          (Number(value) === 1 ? " selected" : "") + '>1</option></select>'
        : '<input data-input-key="' + esc(field.key) + '" value="' + esc(value) + '"' + disabled + '>';
      return '<div class="wb-field"><label>' + esc(field.key) + " · " + esc(field.type) +
        '</label>' + control + '<div class="wb-help">' + esc(field.description || "") + '</div></div>';
    }).join("") +

    '<div class="wb-field"><label>Исполнитель</label><select id="wb-backend"' + (openSession ? " disabled" : "") + '>' +
    BACKENDS.map(([id, label]) => {
      const supported = (state.manifest.supportedИсполнительs || []).includes(id);
      return '<option value="' + id + '"' + (id === state.backend ? " selected" : "") +
        (supported ? "" : " disabled") + '>' + esc(label + (supported ? "" : " — не поддерживается")) + '</option>';
    }).join("") + '</select><div class="wb-help">Неподдерживаемые исполнители показаны явно; скрытого переключения на другой исполнитель нет.</div></div>' +

    '<div class="wb-actions"><button id="wb-open"' + (openSession || state.loading ? " disabled" : "") +
    '>Открыть апамять</button><button id="wb-run" class="primary"' + (!openSession || state.loading ? " disabled" : "") +
    '>Выполнить</button><button id="wb-close"' + (!openSession || state.loading ? " disabled" : "") + '>Закрыть</button></div>' +
    '<div class="wb-help">Готовый и ручной режим используют один и тот же манифест сценария. Изменение входов сохраняет эту же сессию и уже загруженную апамять.</div>' +
    (state.error ? '<p class="wb-error">' + esc(state.error) + '</p>' : '') + '</div>' +

    '<div class="wb-card wb-main"><h3>Одна постоянная апамять</h3>' +
    '<div class="wb-simple-grid">' +
    '<div class="wb-simple-item"><small>Загружено</small><strong>' +
      esc(state.manifest.title || state.manifest.scenarioId || state.manifest.programProfileId || "Сценарий") +
    '</strong></div>' +
    '<div class="wb-simple-item"><small>Входы</small><code>' + esc(JSON.stringify(state.inputs)) + '</code></div>' +
    '<div class="wb-simple-item"><small>Текущее состояние</small><strong>' +
      esc(!status ? "апамять закрыта" : run ? "запуск №" + run.sessionRunId + " завершён" : "загружено один раз · готово к выполнению") +
    '</strong></div></div>' +
    (level === "simple" ? "" :
      '<div class="wb-memory">' +
      '<div class="wb-metric"><small>Сессия</small><code>' + esc(status && status.sessionId || "закрыта") + '</code></div>' +
      '<div class="wb-metric"><small>Хранилище / движок</small><code>' + esc(compact(status && status.storeInstanceId)) +
      " / " + esc(compact(status && status.engineInstanceId)) + '</code></div>' +
      '<div class="wb-metric"><small>Связи Links: база / сейчас</small><strong>' + esc(status && status.baseLinkCount || "—") +
      " / " + esc(currentLinks) + '</strong></div>' +
      '<div class="wb-metric"><small>Подготовка / загрузка / запуски</small><strong>' + esc(status && status.prepareCount || 0) +
      " / " + esc(status && status.loadCount || 0) + " / " + esc(status && status.completedRuns || 0) + '</strong></div>' +
      '</div>') +
    '<div class="wb-result ' + (run ? resultStatus.kind : "") + '"><h4>' + esc(resultStatus.label) + '</h4><div class="wb-result-value">' +
    esc(resultText(run)) + '</div><div class="wb-help">' +
    esc(resultStatus.explanation +
      (level === "simple" || !run ? "" :
        " · " + (run.configurationReused ? "каноническая конфигурация использована повторно" : "опубликованы связи Links конфигурации") +
        " · достигнут покой=" + String(run.observed && run.observed.finalQuiescent))) +
    '</div>' + (run && level !== "simple" ? '<details class="wb-raw"><summary>Нормализованный результат</summary><pre>' + json(run.result) +
    '</pre></details>' : '') + '</div>' +
    (level === "simple" ? '<div class="wb-help">Откройте «Инженерный» режим для связей Links, области Scope, реакций и профиля; «Доказательство» — для сырых структурных данных.</div>' :
      stageDetail ? '<details class="wb-raw" open><summary>Стадия «' + esc(stageLabel(state.stage)) + '» · реальные данные</summary><pre>' +
        json(stageDetail) + '</pre></details>' : '<div class="wb-help">Выберите завершённую стадию конвейера, чтобы посмотреть её реальные данные.</div>') +
    '</div></section>' +

    (level === "simple"
      ? '<section class="wb-card wb-tabs"><div class="wb-help">Простой режим скрывает инженерные и доказательные подробности. Результат выше всё равно получен реальным исполнением.</div></section>'
      : '<section class="wb-card wb-tabs"><div class="wb-tabbar">' +
        (level === "proof"
          ? ["timeline", "log", "profile", "compare", "proof"]
          : ["timeline", "log", "profile", "compare"]).map((name) =>
          '<button data-tab="' + name + '" aria-selected="' + (state.tab === name) + '">' +
          esc(tabLabel(name)) + '</button>').join("") +
        '</div><div class="wb-tab">' + tab(state) + '</div></section>') +
    '</div>';

  const root = state.root;
  for (const button of root.querySelectorAll("[data-level]")) {
    button.addEventListener("click", () => {
      state.level = button.dataset.level;
      if (state.level === "proof") state.tab = "proof";
      else if (state.tab === "proof") state.tab = "timeline";
      render(state);
    });
  }
  for (const button of root.querySelectorAll("[data-stage]")) {
    button.addEventListener("click", () => {
      state.stage = button.dataset.stage;
      if (state.level === "simple") state.level = "engineering";
      render(state);
    });
  }
  root.querySelector("#wb-scenario")?.addEventListener("change", (event) => {
    try { loadManifest(state, Number(event.target.value)); } catch (error) {
      state.error = "Не удалось загрузить сценарий: " + errorText(error);
    }
    render(state);
  });
  root.querySelector("#wb-preset-run")?.addEventListener("change", (event) => {
    state.presetIndex = Number(event.target.value); presetInputs(state); render(state);
  });
  for (const button of root.querySelectorAll("[data-mode]")) {
    button.addEventListener("click", () => {
      syncInputs(state); state.mode = button.dataset.mode;
      if (state.mode === "preset") presetInputs(state);
      render(state);
    });
  }
  root.querySelector("#wb-backend")?.addEventListener("change", (event) => {
    state.backend = event.target.value; render(state);
  });
  for (const input of root.querySelectorAll("[data-input-key]")) {
    input.addEventListener("change", () => { state.inputs[input.dataset.inputKey] = input.value; });
  }
  root.querySelector("#wb-open")?.addEventListener("click", () => void open(state));
  root.querySelector("#wb-run")?.addEventListener("click", () => { syncInputs(state); void execute(state); });
  root.querySelector("#wb-close")?.addEventListener("click", () => close(state));
  for (const button of root.querySelectorAll("[data-tab]")) {
    button.addEventListener("click", () => { state.tab = button.dataset.tab; render(state); });
  }
}

export async function mountWorkbench(root) {
  styles();
  const state = {
    root, wasm: null, registry: { entries: [] }, manifest: { runSequence: [], inputSchema: [] },
    scenarioIndex: 0, presetIndex: 0, inputs: {}, mode: "preset", backend: "optimized-cpu",
    session: null, run: null, history: null, tab: "timeline",
    level: "simple", stage: "RESULT",
    build: { version: "загрузка", sha: "загрузка" }, loading: true, error: null,
  };
  root.innerHTML = '<div class="notice">Загрузка реальной рабочей лаборатории апамяти…</div>';
  try {
    const loaded = await Promise.all([wasm(), buildInfo()]);
    state.wasm = loaded[0]; state.build = loaded[1];
    state.registry = refreshScenarioPresetRegistry(state.wasm);
    let index = state.registry.entries.findIndex((entry) => entry.scenarioId === "mux1-lifecycle");
    if (index < 0) index = 0;
    loadManifest(state, index);
    state.loading = false; render(state);
    await open(state);
  } catch (error) {
    state.loading = false; state.error = "Не удалось запустить рабочую лабораторию: " + errorText(error); render(state);
  }
  return state;
}

if (typeof document !== "undefined") {
  const boot = () => {
    const root = document.querySelector("#workbench-root");
    if (root) void mountWorkbench(root);
  };
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", boot, { once: true });
  else boot();
}
