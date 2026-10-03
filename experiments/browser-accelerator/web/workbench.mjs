import { ScenarioWorkerClient } from "./scenario-worker-client.mjs";
import {
  normalizeCpuScenarioRunV2,
  normalizeWebGpuResidentRunV2,
} from "./run-observation.mjs";
import {
  loadScenarioPresetManifestByIndex,
  refreshScenarioPresetRegistry,
} from "./scenario-presets.mjs";
import { recursiveStructureHtml } from "./proof-view.mjs";
import { PROOF_VERIFICATION_PROFILE_ID } from "./proof-verifier.mjs";

const BACKENDS = [
  ["optimized-cpu", "CPU · оптимизированный"],
  ["webgpu", "WebGPU"],
  ["linksdb", "LinksDB"],
];

const WORKBENCH_HOST_BACKENDS = new Set([
  "optimized-cpu",
]);

export function workbenchBackendAvailability(manifest, backendId) {
  const scenarioSupported =
    (manifest?.supportedBackends || []).includes(backendId);
  const sessionHostSupported = WORKBENCH_HOST_BACKENDS.has(backendId);
  return Object.freeze({
    scenarioSupported,
    sessionHostSupported,
    executable: scenarioSupported && sessionHostSupported,
  });
}

export function workbenchDefaultBackend(manifest) {
  return BACKENDS
    .map(([id]) => id)
    .find((id) => workbenchBackendAvailability(manifest, id).executable)
    ?? null;
}

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

const VERIFICATION_LEVEL_SPECS = Object.freeze([
  ["TRANSPORT_VALID", "transportValid",
    "Проверены формат, границы, декодирование и перенос компактного доказательства."],
  ["TRACE_CONSISTENT", "traceConsistent",
    "Проверена внутренняя согласованность цепочки Scope и результата."],
  ["SEMANTIC_REPLAY_VERIFIED", "semanticReplayVerified",
    "Независимое структурное переисполнение воспроизвело исполнение."],
]);
const verificationState = (value) =>
  value === true ? "verified" : value === false ? "failed" : "unavailable";

export function workbenchVerificationLevels(run, compactVerification = null) {
  const supportedCompactProfile =
    compactVerification?.profileId === PROOF_VERIFICATION_PROFILE_ID;
  const levels = VERIFICATION_LEVEL_SPECS.map(([id, field, detail]) => ({
    id,
    state: verificationState(
      supportedCompactProfile ? compactVerification?.[field] : undefined
    ),
    detail,
    source: "compact-proof",
    profileId: supportedCompactProfile ? compactVerification.profileId : null,
  }));
  levels.push({
    id: "SCALAR_ORACLE_VERIFIED",
    state: verificationState(run?.scalarOracleMatches),
    detail: "Независимая обычная скалярная семантика совпала с результатом апамяти.",
    source: "scenario-scalar-oracle",
    profileId: null,
  });
  return levels;
}

const clone = (value) => JSON.parse(JSON.stringify(value));
const sameInputs = (a, b) => JSON.stringify(a || {}) === JSON.stringify(b || {});

function parseUnsigned(raw, max, label) {
  const text = String(raw).trim();
  const isHex = /^0x[0-9a-f]+$/i.test(text);
  const isDecimal = /^\d+$/.test(text);
  if (!isHex && !isDecimal) {
    throw new Error(
      label + ": требуется целое число от 0 до " + max +
      " без лишних символов",
    );
  }
  const value = isHex
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
      out[field.key] = parseUnsigned(value, 1, field.key);
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

export function createWorkbenchStepRun(
  manifest,
  presetIndex,
  rawInputs,
  ordinal,
  mode = "preset",
) {
  const run = createWorkbenchRun(
    manifest,
    presetIndex,
    rawInputs,
    ordinal,
    mode,
  );
  run.executionMode = "STEP";
  return run;
}

export function workbenchStepSnapshot(stepState) {
  const reports = Array.isArray(stepState?.reports)
    ? stepState.reports
    : [];
  const latest = reports.at(-1) ?? null;
  return {
    active: Boolean(stepState?.active),
    sessionRunId:
      latest?.sessionRunId ?? stepState?.begin?.sessionRunId ?? null,
    reactionCount: reports.length,
    activeReactionCount: latest?.activeReactionCount ?? 0,
    scopeBefore: latest?.evidence?.scopeBefore ?? [],
    scopeAfter: latest?.evidence?.scopeAfter ?? [],
    linksBefore: latest?.evidence?.linksBefore ?? null,
    linksAfter: latest?.evidence?.linksAfter ?? null,
    rawRuleMatches: latest?.evidence?.rawRuleMatches ?? null,
    transitionedMembers: latest?.evidence?.transitionedMembers ?? null,
    handoffCount: latest?.evidence?.handoffCount ?? null,
    quiescent: latest?.evidence?.quiescent ?? false,
    completed: latest?.completed === true,
  };
}

const CONTEXT_FACT_KINDS = new Set([
  "CONTEXT_CREATED",
  "CONTEXT_UPDATED",
  "CONTEXT_PUBLISHED",
  "CONTEXT_COLLAPSED",
]);

function contextFactLabel(kind) {
  return ({
    CONTEXT_CREATED: "Context создан",
    CONTEXT_UPDATED: "Context дополнен",
    CONTEXT_PUBLISHED: "Context опубликовал результат",
    CONTEXT_COLLAPSED: "Context схлопнут",
    CONTEXT_UNAVAILABLE: "Данные Context недоступны",
    REACTION_COMMIT: "Реакция завершена",
  })[kind] || kind;
}

function cloneContext(value) {
  return value == null ? null : clone(value);
}

function reactionPersistentFacts(evidence) {
  const facts = Array.isArray(evidence?.structuralFacts)
    ? evidence.structuralFacts
    : [];
  return {
    createdLinks: facts
      .filter((fact) => fact.kind === "INSTANTIATED")
      .flatMap((fact) => Array.isArray(fact.createdLinks)
        ? fact.createdLinks
        : []),
    publishedOutputs: facts
      .filter((fact) =>
        fact.kind === "PUBLISHED" && fact.preserved === false)
      .flatMap((fact) => Array.isArray(fact.outputs) ? fact.outputs : []),
  };
}

export function workbenchTraceFrames(stepState, mode = "reaction") {
  const reports = Array.isArray(stepState?.reports)
    ? stepState.reports
    : [];
  if (mode !== "reaction" && mode !== "context") {
    throw new Error("Неизвестный режим проигрывателя: " + mode);
  }

  if (mode === "reaction") {
    return reports.map((report, reportIndex) => {
      const evidence = report?.evidence || {};
      const persistent = reactionPersistentFacts(evidence);
      const facts = Array.isArray(evidence.structuralFacts)
        ? evidence.structuralFacts
        : [];
      const contexts = facts
        .filter((fact) => fact.kind === "CONTEXT_CREATED")
        .map((fact) => ({
          id: fact.contextId,
          groupId: fact.contextGroupId,
          parentContextIds: fact.parentContextIds || [],
          active: fact.active,
          rule: fact.rule,
          outputBundleTemplate: fact.outputBundleTemplate,
        }));
      return {
        mode: "reaction",
        kind: "REACTION_COMMIT",
        label: "Реакция " + (reportIndex + 1),
        reportIndex,
        reactionIndex: evidence.reactionIndex ?? reportIndex,
        structuralFactIndex: null,
        locator:
          "run " + (report?.sessionRunId ?? evidence.runId ?? "?") +
          " / reaction " + (evidence.reactionIndex ?? reportIndex),
        currentScope: clone(evidence.scopeAfter || []),
        scopeBefore: clone(evidence.scopeBefore || []),
        scopeAfter: clone(evidence.scopeAfter || []),
        linksBefore: evidence.linksBefore ?? null,
        linksAfter: evidence.linksAfter ?? null,
        rawRuleMatches: evidence.rawRuleMatches ?? null,
        quiescent: evidence.quiescent === true,
        persistentCreatedLinks: clone(persistent.createdLinks),
        publishedOutputs: clone(persistent.publishedOutputs),
        activeContexts: [],
        contexts,
        selectedContext: null,
        result: report?.completed ? clone(report.result) : null,
        completed: report?.completed === true,
        contextAvailable:
          evidence.rawRuleMatches === 0 ||
          contexts.length === (evidence.rawRuleMatches ?? 0),
      };
    });
  }

  const frames = [];
  const activeContexts = new Map();
  for (let reportIndex=0; reportIndex<reports.length; reportIndex+=1) {
    const report=reports[reportIndex];
    const evidence=report?.evidence || {};
    const facts=Array.isArray(evidence.structuralFacts)
      ? evidence.structuralFacts
      : [];
    const persistentCreatedLinks=[];
    const publishedOutputs=[];
    let contextFactCount=0;

    for (let factIndex=0; factIndex<facts.length; factIndex+=1) {
      const fact=facts[factIndex];
      if (fact.kind === "INSTANTIATED") {
        persistentCreatedLinks.push(
          ...(Array.isArray(fact.createdLinks) ? clone(fact.createdLinks) : [])
        );
      }
      if (fact.kind === "PUBLISHED" && fact.preserved === false) {
        publishedOutputs.push(
          ...(Array.isArray(fact.outputs) ? clone(fact.outputs) : [])
        );
      }
      if (!CONTEXT_FACT_KINDS.has(fact.kind)) continue;
      contextFactCount+=1;

      let selectedContext=null;
      if (fact.kind === "CONTEXT_CREATED") {
        selectedContext={
          id:fact.contextId,
          groupId:fact.contextGroupId,
          parentContextIds:clone(fact.parentContextIds || []),
          active:fact.active,
          rule:fact.rule,
          outputBundleTemplate:fact.outputBundleTemplate,
          groundedBundle:null,
          outputs:[],
        };
        activeContexts.set(fact.contextId,selectedContext);
      } else {
        const current=activeContexts.get(fact.contextId);
        selectedContext=current ? cloneContext(current) : {
          id:fact.contextId,
          groupId:fact.contextGroupId,
          parentContextIds:[],
          active:null,
          rule:null,
          outputBundleTemplate:null,
          groundedBundle:null,
          outputs:[],
        };
        if (fact.kind === "CONTEXT_UPDATED") {
          selectedContext.groundedBundle=fact.groundedBundle;
          activeContexts.set(fact.contextId,selectedContext);
        } else if (fact.kind === "CONTEXT_PUBLISHED") {
          selectedContext.outputs=clone(fact.outputs || []);
          activeContexts.set(fact.contextId,selectedContext);
        } else if (fact.kind === "CONTEXT_COLLAPSED") {
          activeContexts.delete(fact.contextId);
        }
      }

      frames.push({
        mode:"context",
        kind:fact.kind,
        label:contextFactLabel(fact.kind),
        reportIndex,
        reactionIndex:evidence.reactionIndex ?? reportIndex,
        structuralFactIndex:factIndex,
        locator:
          "run " + (report?.sessionRunId ?? evidence.runId ?? "?") +
          " / reaction " + (evidence.reactionIndex ?? reportIndex) +
          " / structuralFacts[" + factIndex + "]",
        currentScope:clone(evidence.scopeBefore || []),
        scopeBefore:clone(evidence.scopeBefore || []),
        scopeAfter:clone(evidence.scopeAfter || []),
        linksBefore:evidence.linksBefore ?? null,
        linksAfter:evidence.linksAfter ?? null,
        rawRuleMatches:evidence.rawRuleMatches ?? null,
        quiescent:false,
        persistentCreatedLinks:clone(persistentCreatedLinks),
        publishedOutputs:clone(publishedOutputs),
        activeContexts:[...activeContexts.values()].map(cloneContext),
        selectedContext:cloneContext(selectedContext),
        result:null,
        completed:false,
        contextAvailable:true,
      });
    }

    if ((evidence.rawRuleMatches ?? 0)>0 && contextFactCount===0) {
      frames.push({
        mode:"context",
        kind:"CONTEXT_UNAVAILABLE",
        label:contextFactLabel("CONTEXT_UNAVAILABLE"),
        reportIndex,
        reactionIndex:evidence.reactionIndex ?? reportIndex,
        structuralFactIndex:null,
        locator:
          "run " + (report?.sessionRunId ?? evidence.runId ?? "?") +
          " / reaction " + (evidence.reactionIndex ?? reportIndex),
        currentScope:clone(evidence.scopeBefore || []),
        scopeBefore:clone(evidence.scopeBefore || []),
        scopeAfter:clone(evidence.scopeAfter || []),
        linksBefore:evidence.linksBefore ?? null,
        linksAfter:evidence.linksAfter ?? null,
        rawRuleMatches:evidence.rawRuleMatches ?? null,
        quiescent:false,
        persistentCreatedLinks:clone(persistentCreatedLinks),
        publishedOutputs:clone(publishedOutputs),
        activeContexts:[],
        selectedContext:null,
        result:null,
        completed:false,
        contextAvailable:false,
      });
    }

    frames.push({
      mode:"context",
      kind:"REACTION_COMMIT",
      label:contextFactLabel("REACTION_COMMIT"),
      reportIndex,
      reactionIndex:evidence.reactionIndex ?? reportIndex,
      structuralFactIndex:null,
      locator:
        "run " + (report?.sessionRunId ?? evidence.runId ?? "?") +
        " / reaction " + (evidence.reactionIndex ?? reportIndex) +
        " / commit",
      currentScope:clone(evidence.scopeAfter || []),
      scopeBefore:clone(evidence.scopeBefore || []),
      scopeAfter:clone(evidence.scopeAfter || []),
      linksBefore:evidence.linksBefore ?? null,
      linksAfter:evidence.linksAfter ?? null,
      rawRuleMatches:evidence.rawRuleMatches ?? null,
      quiescent:evidence.quiescent === true,
      persistentCreatedLinks:clone(persistentCreatedLinks),
      publishedOutputs:clone(publishedOutputs),
      activeContexts:[...activeContexts.values()].map(cloneContext),
      selectedContext:null,
      result:report?.completed ? clone(report.result) : null,
      completed:report?.completed === true,
      contextAvailable:
        (evidence.rawRuleMatches ?? 0)===0 || contextFactCount>0,
    });
  }
  return frames;
}

function defaultWorkbenchPlayer() {
  return {
    mode:"reaction",
    cursor:-1,
    playing:false,
    speedMs:800,
    overlays:{
      persistent:true,
      context:true,
      result:true,
    },
  };
}

export function workbenchPlayerSnapshot(stepState) {
  const player=stepState?.player || defaultWorkbenchPlayer();
  const frames=workbenchTraceFrames(stepState,player.mode);
  const cursor=frames.length===0
    ? -1
    : Math.max(0,Math.min(
      Number.isInteger(player.cursor) ? player.cursor : frames.length-1,
      frames.length-1,
    ));
  const frame=cursor>=0 ? frames[cursor] : null;
  return {
    mode:player.mode,
    cursor,
    frameCount:frames.length,
    frame,
    hasPrevious:cursor>0,
    hasCachedNext:cursor>=0 && cursor<frames.length-1,
    canAdvance:
      (cursor>=0 && cursor<frames.length-1) ||
      Boolean(stepState?.active),
    playing:Boolean(player.playing),
    speedMs:player.speedMs || 800,
    overlays:{
      persistent:player.overlays?.persistent !== false,
      context:player.overlays?.context !== false,
      result:player.overlays?.result !== false,
    },
  };
}


function completedStepRun(stepState) {
  const latest = stepState?.reports?.at(-1);
  if (!latest?.completed) return null;
  return {
    sessionRunId: latest.sessionRunId,
    configurationReused: stepState.begin?.configurationReused ?? false,
    linksBeforeConfigure: stepState.begin?.linksBeforeConfigure ?? null,
    linksAfterConfigure: stepState.begin?.linksAfterConfigure ?? null,
    result: latest.result,
    assertionResults: latest.assertionResults ?? [],
    freshInstanceMatches: latest.freshInstanceMatches ?? null,
    scalarOracleMatches: latest.scalarOracleMatches ?? null,
    observed: {
      sessionId: latest.evidence?.sessionId ?? null,
      runId: latest.sessionRunId,
      finalScope: latest.evidence?.scopeAfter ?? [],
      activeReactionCount: latest.activeReactionCount ?? 0,
      finalQuiescent: latest.evidence?.quiescent === true,
      events: stepState.reports.map((report) => ({
        kind: "REACTION_END",
        reactionIndex: report.evidence?.reactionIndex ?? null,
        scopeBefore: report.evidence?.scopeBefore ?? [],
        scopeAfter: report.evidence?.scopeAfter ?? [],
        linksAfter: report.evidence?.linksAfter ?? null,
        rawRuleMatches: report.evidence?.rawRuleMatches ?? null,
        transitionedMembers: report.evidence?.transitionedMembers ?? null,
        handoffCount: report.evidence?.handoffCount ?? null,
        quiescent: report.evidence?.quiescent ?? false,
        structuralFacts: report.evidence?.structuralFacts ?? null,
      })),
      profile: null,
    },
    pipelineProfile: null,
    executionMode: "STEP",
    steppedLive: true,
  };
}

export function deriveWorkbenchPipeline(status, run, stepState = null) {
  const open = Boolean(status);
  const snapshot = workbenchStepSnapshot(stepState);
  const configured = Boolean(run || stepState?.begin);
  const executed = Boolean(run || snapshot.reactionCount > 0);
  const done = Boolean(
    run && (run.pipelineProfile?.stages || run.steppedLive),
  );
  return [
    ["PREPARE", open && status.prepareCount === 1, open ? "prepareCount=" + status.prepareCount : "сессия не открыта"],
    ["LOAD", open && status.loadCount === 1, open ? "базовых связей Links=" + status.baseLinkCount : "сессия не открыта"],
    ["CONFIGURE", configured, configured
      ? "связи Links " + (run?.linksBeforeConfigure ?? stepState?.begin?.linksBeforeConfigure) +
        " → " + (run?.linksAfterConfigure ?? stepState?.begin?.linksAfterConfigure)
      : "запуск ещё не настроен"],
    ["EXECUTE", executed, executed
      ? "реакций=" + (run?.observed?.activeReactionCount ?? snapshot.activeReactionCount) +
        (snapshot.active ? " · пошагово" : "")
      : "исполнение ещё не начато"],
    ["RESULT", done, done ? "получен реальный результат исполнения" :
      snapshot.active ? "результат появится только после покоя" : "запуск ещё не завершён"],
    ["EVIDENCE", executed, executed
      ? "реальных шагов=" + (run?.observed?.events?.length ?? snapshot.reactionCount)
      : "доказательства исполнения ещё не получены"],
  ].map(([id, complete, detail]) => ({
    id,
    state: complete ? "done" : "waiting",
    detail,
  }));
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
    label: passed
      ? "ПРОВЕРКИ СЦЕНАРИЯ ПРОЙДЕНЫ"
      : "ПРОВЕРКИ СЦЕНАРИЯ НЕ ПРОЙДЕНЫ",
    explanation: passed
      ? "Результат исполнения удовлетворяет всем проверкам готового сценария. Это статус проверок сценария, а не общий вердикт доказательства."
      : "Хотя бы одна проверка готового сценария не совпала с реальным результатом исполнения. Уровни доказательства показаны отдельно.",
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
      freshInstanceMatches: run.freshInstanceMatches ?? null,
      scalarOracleMatches: run.scalarOracleMatches ?? null,
      verificationLevels: workbenchVerificationLevels(run),
    };
  }
  if (stageId === "EVIDENCE") {
    return {
      sessionRunId: run.sessionRunId,
      eventCount: run.observed?.events?.length ?? 0,
      observationLevel: run.observed?.observationLevel ?? null,
      pipelineProfile: run.pipelineProfile ?? null,
      verificationLevels: workbenchVerificationLevels(run),
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

function verificationLevelsHtml(run, compactVerification = null) {
  const stateLabels = {
    verified: "ПОДТВЕРЖДЕНО",
    failed: "НЕ ПРОШЛО",
    unavailable: "НЕТ ДАННЫХ",
  };
  return '<div class="wb-verification">' +
    workbenchVerificationLevels(run, compactVerification).map((item) =>
      '<div class="wb-verification-item ' + item.state + '">' +
      '<code>' + esc(item.id) + '</code><strong>' +
      esc(stateLabels[item.state]) + '</strong><small>' +
      esc(item.detail) + '</small></div>'
    ).join("") + '</div>';
}

function freshInstanceEvidenceHtml(run) {
  const value = run?.freshInstanceMatches;
  const state = value === true ? "СОВПАЛО" :
    value === false ? "НЕ СОВПАЛО" : "НЕТ ДАННЫХ";
  return '<div class="wb-help"><code>FRESH_INSTANCE_MATCH</code> · ' +
    esc(state) +
    " · повтор тем же структурным исполнителем; это проверка жизненного цикла, " +
    "а не независимый семантический оракул.</div>";
}

function contextCardHtml(context) {
  if (!context) return "";
  return '<div class="wb-context-card"><strong>Context #' +
    esc(context.id ?? "?") + '</strong><small>группа ' +
    esc(context.groupId ?? "?") + ' · родители ' +
    esc(JSON.stringify(context.parentContextIds || [])) +
    '</small><code>active=' + esc(context.active ?? "—") +
    ' · rule=' + esc(context.rule ?? "—") +
    '</code><small>grounded=' +
    esc(context.groundedBundle ?? "—") + ' · outputs=' +
    esc(JSON.stringify(context.outputs || [])) + '</small></div>';
}

function workbenchPlayerHtml(stepState) {
  const snapshot=workbenchPlayerSnapshot(stepState);
  const frame=snapshot.frame;
  const player=stepState?.player || defaultWorkbenchPlayer();
  if (!frame) {
    return '<div class="wb-step-live"><h4>Проигрыватель исполнения</h4>' +
      '<div class="wb-help">Нажмите «Вперёд»: следующий кадр будет получен настоящим Session.step(), а не сгенерирован интерфейсом.</div>' +
      '<div class="wb-player-controls"><button id="wb-step-next" class="primary"' +
      (!stepState?.active ? ' disabled' : '') + '>Вперёд</button></div></div>';
  }

  const activeContexts=frame.activeContexts || [];
  const contextLayer=frame.contextAvailable
    ? (activeContexts.length
      ? activeContexts.map(contextCardHtml).join("")
      : '<div class="wb-help">Активных временных Context после этого события нет: строительные леса схлопнуты.</div>')
    : '<div class="wb-help"><strong>Данные Context недоступны.</strong> Интерфейс не восстанавливает их из Scope или Links.</div>';

  return '<div class="wb-step-live"><h4>Проигрыватель реального исполнения</h4>' +
    '<div class="wb-player-toolbar">' +
    '<div class="wb-mode"><button data-player-mode="reaction" aria-pressed="' +
      (snapshot.mode==="reaction") + '">Реакции</button>' +
    '<button data-player-mode="context" aria-pressed="' +
      (snapshot.mode==="context") + '">Контексты</button></div>' +
    '<div class="wb-player-controls">' +
      '<button id="wb-player-prev"' + (!snapshot.hasPrevious ? ' disabled' : '') + '>Назад</button>' +
      '<button id="wb-step-next" class="primary"' + (!snapshot.canAdvance ? ' disabled' : '') + '>Вперёд</button>' +
      '<button id="wb-player-play"' + (!snapshot.canAdvance || snapshot.playing ? ' disabled' : '') + '>Воспроизвести</button>' +
      '<button id="wb-player-pause"' + (!snapshot.playing ? ' disabled' : '') + '>Пауза</button>' +
      '<button id="wb-player-end"' + (!stepState?.active ? ' disabled' : '') + '>До конца</button>' +
    '</div><label class="wb-player-speed">Скорость <select id="wb-player-speed">' +
      [250,500,800,1500].map((value) =>
        '<option value="' + value + '"' +
        (snapshot.speedMs===value ? ' selected' : '') + '>' +
        value + ' мс</option>').join("") +
      '</select></label></div>' +
    '<div class="wb-player-toolbar"><div class="wb-player-controls">' +
      '<button id="wb-jump-create">К созданию Context</button>' +
      '<button id="wb-jump-collapse">К схлопыванию Context</button>' +
    '</div><div class="wb-overlays">' +
      '<label><input type="checkbox" data-overlay="persistent"' +
        (snapshot.overlays.persistent ? ' checked' : '') + '> Постоянная асеть</label>' +
      '<label><input type="checkbox" data-overlay="context"' +
        (snapshot.overlays.context ? ' checked' : '') + '> Временный Context</label>' +
      '<label><input type="checkbox" data-overlay="result"' +
        (snapshot.overlays.result ? ' checked' : '') + '> Result</label>' +
    '</div></div>' +
    '<div class="wb-player-head"><strong>' + esc(frame.label) +
      '</strong><span>кадр ' + esc(snapshot.cursor+1) + '/' +
      esc(snapshot.frameCount) + '</span><code>Источник evidence: ' +
      esc(frame.locator) + '</code></div>' +
    '<div class="wb-simple-grid">' +
      '<div class="wb-simple-item"><small>Текущий Scope</small><code>' +
        esc(JSON.stringify(frame.currentScope || [])) + '</code></div>' +
      '<div class="wb-simple-item"><small>Границы Links реакции</small><strong>' +
        esc(frame.linksBefore ?? "—") + ' → ' +
        esc(frame.linksAfter ?? "—") + '</strong></div>' +
      '<div class="wb-simple-item"><small>Группа / событие</small><strong>' +
        esc(frame.selectedContext?.groupId ?? "—") + ' / ' +
        esc(frame.kind) + '</strong></div></div>' +
    (snapshot.overlays.persistent
      ? '<section class="wb-layer wb-layer-persistent"><h5>ПОСТОЯННАЯ АСЕТЬ / STORE</h5>' +
        '<div class="wb-help">Физически созданные Links в этой реакции до выбранного события: ' +
        esc(frame.persistentCreatedLinks?.length || 0) +
        '. Они не являются Context и не исчезают при его схлопывании.</div>' +
        '<code>' + esc(JSON.stringify(frame.persistentCreatedLinks || [])) +
        '</code><div class="wb-help">Опубликованные выходы: ' +
        esc(JSON.stringify(frame.publishedOutputs || [])) + '</div></section>'
      : '') +
    (snapshot.overlays.context
      ? '<section class="wb-layer wb-layer-context"><h5>ВРЕМЕННЫЙ CONTEXT — строительные леса</h5>' +
        (frame.selectedContext
          ? '<div class="wb-help">Context события:</div>' +
            contextCardHtml(frame.selectedContext)
          : '') +
        contextLayer + '</section>'
      : '') +
    (snapshot.overlays.result
      ? '<section class="wb-layer wb-layer-result"><h5>RESULT / опубликованный результат</h5>' +
        (frame.result
          ? '<pre>' + json(frame.result) + '</pre>'
          : '<div class="wb-help">Финальный Result появляется только после реального достижения покоя. Уже опубликованные persistent outputs при этом не стираются: ' +
            esc(JSON.stringify(frame.publishedOutputs || [])) + '</div>') +
        '</section>'
      : '') +
    '<div class="wb-help">«Назад» меняет только курсор просмотра evidence и никогда не откатывает апамять. «Вперёд» у конца уже полученной трассы вызывает настоящий Session.step(). Sibling Context одной группы атомарны: порядок строк observer не считается семантическим порядком.</div>' +
  '</div>';
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
    ".wb-step-live{margin-top:12px;padding:14px;border:2px solid var(--accent);border-radius:12px;background:var(--surface-2)}.wb-step-live h4{margin:0 0 10px}.wb-step-live h5{margin:0 0 7px}.wb-player-toolbar{display:flex;justify-content:space-between;gap:10px;align-items:center;flex-wrap:wrap;margin:8px 0}.wb-player-controls,.wb-overlays{display:flex;gap:7px;flex-wrap:wrap;align-items:center}.wb-player-controls button,.wb-player-speed select{min-height:34px;border:1px solid var(--line);border-radius:8px;background:var(--surface);color:var(--text);padding:6px 8px}.wb-player-controls .primary{background:var(--accent);color:#fff;border-color:var(--accent)}.wb-player-controls button:disabled{opacity:.45}.wb-player-speed,.wb-overlays label{font-size:.78rem;color:var(--muted)}.wb-player-head{display:grid;grid-template-columns:auto auto minmax(0,1fr);gap:10px;align-items:center;margin:10px 0}.wb-player-head span{color:var(--muted);font-size:.78rem}.wb-player-head code{text-align:right;overflow-wrap:anywhere}.wb-layer{margin-top:9px;padding:11px;border-radius:10px;background:var(--surface)}.wb-layer-persistent{border:2px solid var(--line)}.wb-layer-context{border:2px dashed var(--accent)}.wb-layer-result{border:3px double var(--good)}.wb-layer>code{display:block;max-height:120px;overflow:auto;white-space:pre-wrap;overflow-wrap:anywhere}.wb-context-card{display:grid;gap:3px;padding:8px;margin:6px 0;border:1px dashed var(--accent);border-radius:9px;background:var(--surface-2)}.wb-context-card small{color:var(--muted)}",
    ".wb-verification{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:8px;margin:10px 0}.wb-verification-item{display:grid;gap:5px;padding:10px;border:1px solid var(--line);border-radius:10px;background:var(--surface-2)}.wb-verification-item code{overflow-wrap:anywhere}.wb-verification-item strong{font-size:.76rem}.wb-verification-item small{color:var(--muted)}.wb-verification-item.verified{border-color:var(--good)}.wb-verification-item.failed{border-color:var(--bad)}",
    ".wb-tabs{padding:0 16px 16px}.wb-tabbar{display:flex;gap:6px;flex-wrap:wrap;border-bottom:1px solid var(--line);padding-bottom:9px}.wb-tabbar button{border:0;background:transparent;color:var(--muted);padding:7px 9px;cursor:pointer}.wb-tabbar button[aria-selected=true]{color:var(--text);font-weight:800;border-bottom:2px solid var(--accent)}.wb-tab{padding-top:12px}",
    ".wb-log{display:grid;gap:6px;max-height:360px;overflow:auto}.wb-log-row{padding:8px 10px;border:1px solid var(--line);border-radius:9px;background:var(--surface-2);font-size:.78rem}.wb-table{width:100%;border-collapse:collapse;font-size:.8rem}.wb-table th,.wb-table td{text-align:left;padding:7px 8px;border-bottom:1px solid var(--line)}.wb-table th{color:var(--muted)}",
    ".wb-raw{margin-top:10px;border:1px solid var(--line);border-radius:10px;overflow:hidden}.wb-raw summary{cursor:pointer;padding:9px 11px;font-weight:700}.wb-raw pre{border-radius:0;max-height:360px;font-size:.75rem}.wb-error{color:var(--bad);font-weight:800}",
    ".wb-proof-structure{margin-top:10px;padding:10px;border:1px solid var(--line);border-radius:10px;background:var(--surface-2)}.proof-recursive-structure{margin-top:8px;border:1px solid var(--line);border-radius:9px;background:var(--surface)}.proof-recursive-structure summary{cursor:pointer;padding:8px 10px}.proof-recursive-structure>code{display:block;padding:10px;max-height:280px;overflow:auto;overflow-wrap:anywhere}",
    ".wb-levels{display:flex;gap:5px;flex-wrap:wrap}.wb-levels button{border:1px solid var(--line);border-radius:999px;background:var(--surface-2);color:var(--muted);padding:5px 8px;cursor:pointer;font-size:.76rem}.wb-levels button[aria-pressed=true]{border-color:var(--accent);color:var(--text);font-weight:800}",
    ".wb-simple-grid{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:8px;margin-bottom:12px}.wb-simple-item{padding:12px;border:1px solid var(--line);border-radius:10px;background:var(--surface-2);min-width:0}.wb-simple-item small{display:block;color:var(--muted)}.wb-simple-item strong,.wb-simple-item code{display:block;margin-top:5px;overflow-wrap:anywhere}.wb-result.pass{border-color:var(--good)}.wb-result.fail{border-color:var(--bad)}",
    ".wb-gpu-witness{display:grid;gap:8px;margin-top:12px;padding:12px;border:1px solid var(--line);border-radius:10px;background:var(--surface-2)}.wb-gpu-witness>div:first-child{display:flex;gap:8px;align-items:center;flex-wrap:wrap}.wb-gpu-witness small,.wb-gpu-witness code{display:block;overflow-wrap:anywhere}.wb-gpu-witness button{justify-self:start}",

    "@media(max-width:1000px){.wb-grid{grid-template-columns:1fr}.wb-memory{grid-template-columns:1fr 1fr}.wb-pipe{grid-template-columns:repeat(3,1fr)}}@media(max-width:620px){.wb-memory,.wb-pipe{grid-template-columns:1fr}}",
  ].join("\n");
  document.head.append(style);
}

async function catalogWasm() {
  const response = await fetch("./amemory_a_circuit.wasm", {
    cache: "no-store",
  });
  if (!response.ok) {
    throw new Error(
      "Не удалось загрузить каталог A-Circuit WASM: HTTP " +
      response.status,
    );
  }
  const bytes = await response.arrayBuffer();
  const loaded = await WebAssembly.instantiate(bytes, {});
  const w = loaded.instance.exports;
  for (const name of [
    "amemory_scenario_preset_registry_refresh",
    "amemory_scenario_preset_manifest_load",
  ]) {
    if (typeof w[name] !== "function") {
      throw new Error(
        "В WASM отсутствует функция ABI каталога: " + name,
      );
    }
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


const GPU_WITNESS_RESULT_ABI = Object.freeze({
  available: "amemory_i386_lab_result_available",
  length: "amemory_i386_lab_result_json_len",
  pointer: "amemory_i386_lab_result_json_ptr",
  byte: "amemory_i386_lab_result_json_byte",
});

function requireMux1StaticRoots(metadata, carrier) {
  if (metadata?.schemaVersion !== 1 ||
      metadata?.representationId !== "amemory-i386-lab-result-json" ||
      metadata?.witnessKind !== "gpu-carrier" ||
      metadata?.compactProofAvailable !== false ||
      metadata?.payload?.staticProgram !== true ||
      metadata?.payload?.linkCount !== carrier.layout.linkCount ||
      !Array.isArray(metadata?.payload?.semanticRoots)) {
    throw new Error("WebGPU static PREPARE metadata mismatch");
  }
  const roots = {};
  for (const root of metadata.payload.semanticRoots) {
    if (typeof root?.role !== "string" ||
        !Number.isInteger(root?.carrierRef) ||
        root.carrierRef < 1 ||
        root.carrierRef > carrier.layout.linkCount) {
      throw new Error("WebGPU static PREPARE contains invalid semantic root");
    }
    roots[root.role] = root.carrierRef >>> 0;
  }
  for (const role of [
    "function.mux1",
    "data.zero",
    "data.one",
    "execution.interpreter",
    "execution.theory",
    "execution.apply",
    "context.caller",
    "result.tag",
  ]) {
    if (!Number.isInteger(roots[role])) {
      throw new Error("WebGPU static PREPARE misses root " + role);
    }
  }
  for (const forbidden of [
    "data.select",
    "data.a",
    "data.b",
    "invocation.args",
    "invocation.call",
    "scope.initial",
  ]) {
    if (Object.hasOwn(roots, forbidden)) {
      throw new Error(
        "WebGPU static PREPARE illegally contains runtime root " + forbidden,
      );
    }
  }
  return Object.freeze(roots);
}

function mux1ResidentConfigurationRecipe(carrier, roots, inputs) {
  const bit = (key) => {
    const value = Number(inputs?.[key]);
    if (!Number.isInteger(value) || value < 0 || value > 1) {
      throw new Error("canonical MUX1 Scenario input " + key + " is not BIT");
    }
    return value === 0 ? roots["data.zero"] : roots["data.one"];
  };
  const H = (handle) => ({ handle });
  const O = (operation) => ({ operation });
  const operations = [];
  let sequence = H(carrier.layout.rootHandle);

  for (const key of ["S", "A", "B"]) {
    const pair = operations.length;
    operations.push({
      kind: "PAIR",
      start: sequence,
      end: H(bit(key)),
    });
    const cell = operations.length;
    operations.push({ kind: "START", child: O(pair) });
    sequence = O(cell);
  }

  const functionArgument = operations.length;
  operations.push({
    kind: "PAIR",
    start: H(roots["function.mux1"]),
    end: sequence,
  });
  const invocation = operations.length;
  operations.push({
    kind: "PAIR",
    start: H(roots["execution.apply"]),
    end: O(functionArgument),
  });
  const initial = operations.length;
  operations.push({
    kind: "PAIR",
    start: H(roots["context.caller"]),
    end: O(invocation),
  });

  return Object.freeze({
    interpreterHandle: roots["execution.interpreter"],
    operations: Object.freeze(operations),
    initial: O(initial),
  });
}

function topologyPoles(carrier, residentStarts, residentEnds, handle) {
  if (!Number.isInteger(handle) || handle < 1) return null;
  if (handle <= carrier.layout.linkCount) {
    return [
      carrier.sections.starts[handle - 1] >>> 0,
      carrier.sections.ends[handle - 1] >>> 0,
    ];
  }
  const index = handle - carrier.layout.linkCount - 1;
  if (index < 0 || index >= residentStarts.length ||
      index >= residentEnds.length) {
    return null;
  }
  return [residentStarts[index] >>> 0, residentEnds[index] >>> 0];
}

function readExactSequenceTopology(
  carrier,
  residentStarts,
  residentEnds,
  finalHandle,
) {
  if (finalHandle === carrier.layout.rootHandle) return [];
  const reversed = [];
  const seen = new Set();
  let current = finalHandle;
  while (current !== carrier.layout.rootHandle) {
    if (seen.has(current)) {
      throw new Error("WebGPU result ExactSequence is cyclic");
    }
    seen.add(current);
    const cell = topologyPoles(
      carrier,
      residentStarts,
      residentEnds,
      current,
    );
    if (!cell || cell[0] !== current || cell[1] === current) {
      throw new Error("WebGPU result is not an ExactSequence");
    }
    const payload = topologyPoles(
      carrier,
      residentStarts,
      residentEnds,
      cell[1],
    );
    if (!payload) {
      throw new Error("WebGPU result ExactSequence payload is missing");
    }
    reversed.push(payload[1]);
    current = payload[0];
  }
  reversed.reverse();
  return reversed;
}

function decodeMux1ResidentResult(
  carrier,
  residentStarts,
  residentEnds,
  finalHandle,
  roots,
) {
  const final = topologyPoles(
    carrier,
    residentStarts,
    residentEnds,
    finalHandle,
  );
  if (!final || final[0] !== roots["context.caller"]) {
    throw new Error("WebGPU MUX1 final Scope lost context.caller");
  }
  const endpoint = topologyPoles(
    carrier,
    residentStarts,
    residentEnds,
    final[1],
  );
  if (!endpoint || endpoint[0] !== roots["result.tag"]) {
    throw new Error("WebGPU MUX1 final Scope lost result.tag");
  }
  const values = readExactSequenceTopology(
    carrier,
    residentStarts,
    residentEnds,
    endpoint[1],
  );
  if (values.length !== 1) {
    throw new Error("WebGPU MUX1 result must contain exactly one bit");
  }
  if (values[0] === roots["data.zero"]) return 0;
  if (values[0] === roots["data.one"]) return 1;
  throw new Error("WebGPU MUX1 result is not data.zero/data.one");
}

function manifestAssertion(run, kind, field = null) {
  const assertion = (run?.assertions || []).find((candidate) =>
    candidate?.kind === kind &&
    (field === null || candidate?.field === field)
  );
  if (!assertion) {
    throw new Error(
      "canonical Scenario run " + (run?.runId || "?") +
      " misses assertion " + kind,
    );
  }
  return assertion.expected;
}

export async function runWorkbenchWebGpuWitness(
  catalogWasm,
  sourceSha = null,
) {
  if (!globalThis.navigator?.gpu) {
    return {
      status: "unavailable",
      reason: "WebGPU API недоступен в этом браузере.",
    };
  }
  if (typeof catalogWasm?.amemory_i386_lab_gpu_carrier_prepare !== "function" ||
      catalogWasm.amemory_i386_lab_gpu_carrier_prepare() !== 1) {
    throw new Error("A-Circuit WASM не подготовил static MUX1 packed carrier");
  }

  const suffix = /^[0-9a-f]{40}$/.test(sourceSha || "")
    ? "?v=" + sourceSha
    : "";
  const [gpuModule, jsonModule, presetModule, scenarioModule] =
    await Promise.all([
      import("./gpu-carrier.mjs" + suffix),
      import("./i386-wasm-json.mjs" + suffix),
      import("./scenario-presets.mjs" + suffix),
      import("./scenario-transport.mjs" + suffix),
    ]);

  const carrier = gpuModule.readGpuCarrierWordsAbi(
    catalogWasm,
    undefined,
    "Workbench static MUX1 carrier",
  );
  const metadata = jsonModule.readJsonAbi(
    catalogWasm,
    GPU_WITNESS_RESULT_ABI,
    "Workbench static MUX1 carrier metadata",
  );
  if (!carrier) {
    throw new Error("WebGPU static MUX1 carrier is unavailable");
  }
  const roots = requireMux1StaticRoots(metadata, carrier);

  // The four typed input vectors come only from the canonical embedded Scenario.
  const registry = presetModule.refreshScenarioPresetRegistry(catalogWasm);
  const preset = presetModule.loadScenarioPresetManifest(
    catalogWasm,
    registry,
    "mux1-lifecycle",
    "1.0.0",
  );
  if (!preset) {
    throw new Error("canonical mux1-lifecycle@1.0.0 Scenario is unavailable");
  }
  const manifest = JSON.parse(preset.source);
  if (manifest?.scenarioId !== "mux1-lifecycle" ||
      manifest?.scenarioVersion !== "1.0.0" ||
      manifest?.programProfile?.profileId !== "a-circuit:mux1" ||
      !Array.isArray(manifest?.runSequence) ||
      manifest.runSequence.length !== 4) {
    throw new Error("canonical MUX1 Scenario manifest mismatch");
  }

  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) {
    return {
      status: "unavailable",
      reason: "WebGPU adapter недоступен на этом устройстве.",
    };
  }

  const device = await adapter.requestDevice();
  let resident = null;
  try {
    resident = await gpuModule.openGpuCarrierResidentSession(
      device,
      carrier,
    );

    let residentStarts = [];
    let residentEnds = [];
    let firstInitialHandle = null;
    const gpuRuns = [];

    // GPU execution happens first. CPU/Rust/WASM Scenario execution below is
    // post-readback comparison only and cannot seed Scope/result/append data.
    for (let index = 0; index < manifest.runSequence.length; index += 1) {
      const scenarioRun = manifest.runSequence[index];
      if (scenarioRun.executionMode !== "TO_QUIESCENCE") {
        throw new Error(
          "WebGPU witness supports only canonical TO_QUIESCENCE runs",
        );
      }

      const recipe = mux1ResidentConfigurationRecipe(
        carrier,
        roots,
        scenarioRun.inputs,
      );
      const beforeConfigure = resident.telemetry();
      const linksBeforeConfigure =
        carrier.layout.linkCount + beforeConfigure.residentAppendCount;
      const configured = await resident.configure(recipe);
      const afterConfigure = resident.telemetry();
      const linksAfterConfigure =
        carrier.layout.linkCount + afterConfigure.residentAppendCount;
      residentStarts = [...configured.residentStarts];
      residentEnds = [...configured.residentEnds];

      if (index < 3 && configured.appendCount <= 0) {
        throw new Error(
          "new canonical MUX1 configuration did not publish Links at run " +
          index,
        );
      }
      if (index === 0) {
        firstInitialHandle = configured.initialHandle;
      }
      if (index === 3 &&
          (configured.appendCount !== 0 ||
           configured.initialHandle !== firstInitialHandle)) {
        throw new Error(
          "return-to-first MUX1 configuration was not canonically reused",
        );
      }

      const beforeExecution = resident.telemetry();
      const run = await resident.runToQuiescence(
        {
          currentHandle: configured.initialHandle,
          interpreterHandle: configured.interpreterHandle,
        },
        { maxReactions: scenarioRun.maxReactions ?? 4096 },
      );
      if (run.stopReason !== "QUIESCENT" || run.quiescent !== true) {
        throw new Error(
          "WebGPU Scenario run " + scenarioRun.runId +
          " did not reach real QUIESCENT: " + run.stopReason,
        );
      }

      for (const step of run.steps) {
        const observed = step.observed;
        if (observed.appendCount !== observed.appendStarts.length ||
            observed.appendCount !== observed.appendEnds.length) {
          throw new Error("WebGPU reaction append readback is inconsistent");
        }
        residentStarts.push(...observed.appendStarts);
        residentEnds.push(...observed.appendEnds);
      }

      const value = decodeMux1ResidentResult(
        carrier,
        residentStarts,
        residentEnds,
        run.finalCurrentHandle,
        roots,
      );
      const expectedValue = Number(
        manifestAssertion(
          scenarioRun,
          "RESULT_FIELD_EQUALS",
          "value",
        ),
      );
      const expectedReactions = Number(
        manifestAssertion(
          scenarioRun,
          "REACTION_COUNT_EQUALS",
        ),
      );
      const expectedQuiescent = Boolean(
        manifestAssertion(
          scenarioRun,
          "QUIESCENT_EQUALS",
        ),
      );
      if (value !== expectedValue ||
          run.activeReactionCount !== expectedReactions ||
          run.quiescent !== expectedQuiescent) {
        throw new Error(
          "WebGPU Scenario assertions failed for " + scenarioRun.runId +
          " value=" + value +
          " active=" + run.activeReactionCount +
          " quiescent=" + run.quiescent,
        );
      }

      const afterExecution = resident.telemetry();
      const normalizedObservation = normalizeWebGpuResidentRunV2({
        sessionId: resident.sessionId,
        runId: index + 1,
        manifestRunId: scenarioRun.runId,
        configurationReused: configured.appendCount === 0,
        baseLinkCount: carrier.layout.linkCount,
        linksBeforeConfigure,
        linksAfterConfigure,
        run,
        telemetryBeforeConfigure: beforeConfigure,
        telemetryAfterConfigure: afterConfigure,
        telemetryBeforeExecute: beforeExecution,
        telemetryAfterExecute: afterExecution,
        result: Object.freeze({
          fields: Object.freeze({ value }),
        }),
      });
      gpuRuns.push(Object.freeze({
        manifestRunId: scenarioRun.runId,
        inputs: Object.freeze({ ...scenarioRun.inputs }),
        configurationAppendCount: configured.appendCount,
        configurationReused: configured.appendCount === 0,
        initialHandle: configured.initialHandle,
        finalHandle: run.finalCurrentHandle,
        value,
        activeReactionCount: run.activeReactionCount,
        stepCount: run.steps.length,
        stopReason: run.stopReason,
        executionAppendCount:
          afterExecution.residentAppendCount -
          beforeExecution.residentAppendCount,
        normalizedObservation,
      }));
    }

    // Only after all four GPU results have been read back do we execute the
    // same canonical manifest through the Rust/WASM optimized-CPU Session.
    const cpu = scenarioModule.executeScenarioManifest(
      catalogWasm,
      manifest,
      "optimized-cpu",
    );
    if (!cpu.ok || cpu.report?.overallPass !== true ||
        !Array.isArray(cpu.report?.runs) ||
        cpu.report.runs.length !== gpuRuns.length) {
      throw new Error(
        "post-readback optimized-CPU Scenario differential is unavailable",
      );
    }

    const normalizedCpuRuns = [];
    for (let index = 0; index < gpuRuns.length; index += 1) {
      const gpu = gpuRuns[index];
      const reference = cpu.report.runs[index];
      const normalizedCpu = normalizeCpuScenarioRunV2(reference);
      normalizedCpuRuns.push(normalizedCpu);
      const gpuProfile = gpu.normalizedObservation.profile;
      const cpuProfile = normalizedCpu.profile;
      if (reference?.manifestRunId !== gpu.manifestRunId ||
          Number(reference?.result?.fields?.value) !== gpu.value ||
          reference?.observed?.finalQuiescent !== true ||
          Number(reference?.observed?.activeReactionCount) !==
            gpu.activeReactionCount ||
          gpuProfile.backendId !== "webgpu" ||
          cpuProfile.backendId !== "optimized-cpu" ||
          gpuProfile.stopReason !== "QUIESCENT" ||
          cpuProfile.stopReason !== "QUIESCENT" ||
          gpuProfile.finalQuiescent !== true ||
          cpuProfile.finalQuiescent !== true ||
          gpuProfile.activeReactionCount?.availability !== "MEASURED" ||
          cpuProfile.activeReactionCount?.availability !== "MEASURED" ||
          gpuProfile.activeReactionCount.value !==
            cpuProfile.activeReactionCount.value) {
        throw new Error(
          "post-readback CPU/WebGPU normalized differential failed at " +
          gpu.manifestRunId,
        );
      }
    }

    const residency = resident.telemetry();
    const expectedDispatches = gpuRuns.reduce(
      (sum, run) => sum + run.stepCount,
      0,
    );
    if (residency.baseUploadCount !== 1 ||
        residency.configurationCommitCount !== gpuRuns.length ||
        residency.configurationDispatchCount !== 3 ||
        residency.reactionDispatchCount !== expectedDispatches ||
        residency.closed !== false) {
      throw new Error(
        "WebGPU persistent Scenario accounting mismatch: " +
        JSON.stringify(residency),
      );
    }

    return {
      status: "verified",
      scenarioManifestDriven: true,
      staticPreparedCarrier: true,
      noExecutionSeed: true,
      postReadbackCpuDifferential: true,
      cpuReferenceSharesRustCore: true,
      sequentialResidentExecution: true,
      residentBaseReuse: true,
      returnToFirstReuse:
        gpuRuns[3].configurationReused === true &&
        gpuRuns[3].initialHandle === gpuRuns[0].initialHandle,
      allQuiescent: gpuRuns.every((run) => run.stopReason === "QUIESCENT"),
      manifestAssertionsPassed: true,
      sourceSha: /^[0-9a-f]{40}$/.test(sourceSha || "")
        ? sourceSha
        : null,
      scenarioId: manifest.scenarioId,
      scenarioVersion: manifest.scenarioVersion,
      planMode: resident.plan.mode,
      linkCount: carrier.layout.linkCount,
      runCount: gpuRuns.length,
      values: gpuRuns.map((run) => run.value),
      activeReactionCounts:
        gpuRuns.map((run) => run.activeReactionCount),
      stepCounts: gpuRuns.map((run) => run.stepCount),
      configurationAppendCounts:
        gpuRuns.map((run) => run.configurationAppendCount),
      executionAppendCounts:
        gpuRuns.map((run) => run.executionAppendCount),
      initialHandles: gpuRuns.map((run) => run.initialHandle),
      finalScopeHandles: gpuRuns.map((run) => run.finalHandle),
      residentAppendCount: residency.residentAppendCount,
      residentCapacity: residency.residentCapacity,
      residentBufferBytes: residency.residentBufferBytes,
      baseUploadCount: residency.baseUploadCount,
      baseUploadBytes: residency.baseUploadBytes,
      configurationCommitCount: residency.configurationCommitCount,
      configurationDispatchCount: residency.configurationDispatchCount,
      configurationUploadBytes: residency.configurationUploadBytes,
      reactionDispatchCount: residency.reactionDispatchCount,
      normalizedObservationSchemaVersion: 2,
      normalizedGpuRuns:
        gpuRuns.map((run) => run.normalizedObservation),
      normalizedCpuRuns,
    };
  } finally {
    resident?.close();
    device.destroy?.();
  }
}

async function runGpuWitness(state) {
  state.gpuWitness = { status: "running" };
  render(state);
  try {
    state.gpuWitness = await runWorkbenchWebGpuWitness(
      state.catalogWasm,
      state.build.sha,
    );
  } catch (error) {
    state.gpuWitness = {
      status: "failed",
      reason: errorText(error),
    };
  }
  render(state);
  return state.gpuWitness;
}

function gpuWitnessHtml(state) {
  const witness = state.gpuWitness || { status: "idle" };
  const status = witness.status || "idle";
  const labels = {
    idle: "НЕ ЗАПУЩЕНО",
    running: "ВЫПОЛНЯЕТСЯ",
    verified: "ПОДТВЕРЖДЕНО",
    unavailable: "НЕДОСТУПНО",
    failed: "ОШИБКА",
  };
  const detail = status === "verified"
    ? '<strong>WebGPU ↔ Rust/WASM после readback</strong>' +
      '<small>' + esc(witness.scenarioId) + '@' +
      esc(witness.scenarioVersion) + ' · runs ' +
      esc(witness.runCount) + ' · values ' +
      esc((witness.values || []).join(",")) + ' · active ' +
      esc((witness.activeReactionCounts || []).join(",")) +
      ' · config append ' +
      esc((witness.configurationAppendCounts || []).join(",")) +
      ' · base upload ×' + esc(witness.baseUploadCount) + '</small>' +
      '<code>return-to-first=' + esc(witness.returnToFirstReuse) +
      ' · resident Links ' + esc(witness.residentAppendCount) + '</code>'
    : status === "running"
      ? '<small>Один static packed base загружается в WebGPU; четыре конфигурации берутся из canonical Scenario и исполняются до QUIESCENT.</small>'
      : '<small>' + esc(
          witness.reason ||
          "WebGPU Scenario-свидетель ещё не запускался.",
        ) + '</small>';

  return '<div class="wb-gpu-witness"><div><strong>WebGPU · постоянная Session ×4</strong>' +
    '<span class="wb-chip ' +
    (status === "verified" ? "good" : status === "failed" ? "bad" : "") +
    '">' + esc(labels[status] || status) + '</span></div>' +
    detail +
    '<button id="wb-gpu-witness"' +
    (status === "running" ? " disabled" : "") +
    '>Проверить WebGPU Scenario ×4</button>' +
    '<div class="wb-help">PREPARE/LOAD выполняются один раз. S/A/B берутся только из canonical mux1-lifecycle manifest и публикуются generic PAIR/START CONFIGURE-дельтой. GPU исполняет четыре запуска в одной resident Session; Rust/WASM сравнивается только после GPU readback и не может задавать Scope, successor или append.</div></div>';
}

function presetInputs(state) {
  const run = state.manifest.runSequence[state.presetIndex];
  state.inputs = clone((run && run.inputs) || state.manifest.initialInputs || {});
}

async function loadManifest(state, index) {
  stopWorkbenchPlayer(state);
  if (state.session) {
    await state.worker.closeSession();
  }
  const source = loadScenarioPresetManifestByIndex(
    state.catalogWasm,
    index,
  );
  if (source == null) {
    throw new Error(
      "Не найден манифест сценария с индексом " + index,
    );
  }
  state.scenarioIndex = index;
  state.manifest = JSON.parse(source);
  state.presetIndex = 0;
  state.mode = "preset";
  state.backend = workbenchDefaultBackend(state.manifest) || "";
  state.session = null;
  state.run = null;
  state.step = null;
  state.history = null;
  state.workerProgress = null;
  state.workerRunActive = false;
  state.cancelling = false;
  state.error = null;
  presetInputs(state);
}

async function history(state) {
  if (!state.session) {
    state.history = null;
    return;
  }
  try {
    const refreshed = await state.worker.history();
    state.history = refreshed.status || null;
  } catch {
    state.history = null;
  }
}

async function open(state) {
  stopWorkbenchPlayer(state);
  state.loading = true;
  state.error = null;
  render(state);
  try {
    const availability =
      workbenchBackendAvailability(state.manifest, state.backend);
    if (!availability.scenarioSupported) {
      throw new Error(
        "Сценарий не поддерживает исполнитель " + state.backend,
      );
    }
    if (!availability.sessionHostSupported) {
      throw new Error(
        "Исполнитель " + state.backend +
        " недоступен в браузерном хосте",
      );
    }
    const opened = await state.worker.open(
      state.manifest,
      state.backend,
    );
    state.session = opened.status;
    state.run = null;
    state.step = null;
    state.workerProgress = null;
    state.history = opened.history || null;
  } catch (error) {
    state.session = null;
    state.run = null;
    state.history = null;
    state.error =
      "Не удалось открыть апамять: " + errorText(error);
  }
  state.loading = false;
  render(state);
}

async function execute(state) {
  stopWorkbenchPlayer(state);
  state.loading = true;
  state.workerRunActive = true;
  state.cancelling = false;
  state.workerProgress = null;
  state.error = null;
  render(state);

  try {
    const request = createWorkbenchRun(
      state.manifest,
      state.presetIndex,
      state.inputs,
      (state.session.completedRuns || 0) + 1,
      state.mode,
    );
    const result = await state.worker.run(request, {
      onProgress(progress) {
        state.workerProgress = progress;
        if (progress?.status) state.session = progress.status;
        // Keep the UI responsive without rendering every single reaction.
        if ((progress?.completedSteps || 0) <= 2 ||
            (progress?.completedSteps || 0) % 16 === 0 ||
            progress?.completed === true) {
          render(state);
        }
      },
    });

    state.session = result.status;
    const completed = {
      active: false,
      begin: result.begin,
      reports: result.reports,
    };
    state.run = completedStepRun(completed);
    state.step = null;
    state.stage = "RESULT";
    await history(state);
  } catch (error) {
    if (error?.recovery === "REOPEN_REQUIRED" ||
        error?.recovery === "OPEN_REQUIRED") {
      state.session = null;
      state.step = null;
      state.history = null;
    }
    state.error = "Ошибка исполнения: " + errorText(error);
  }

  state.workerRunActive = false;
  state.cancelling = false;
  state.loading = false;
  render(state);
}

async function cancelRun(state) {
  if (!state.workerRunActive || state.cancelling) return;
  state.cancelling = true;
  render(state);
  try {
    const result = await state.worker.cancelActive();
    if (!result.accepted) {
      state.cancelling = false;
      state.error = "Worker уже не имеет активного запуска для отмены.";
      render(state);
    }
  } catch (error) {
    state.cancelling = false;
    state.error = "Не удалось запросить отмену: " + errorText(error);
    render(state);
  }
}

async function beginStep(state) {
  state.loading = true;
  state.error = null;
  render(state);
  try {
    const request = createWorkbenchStepRun(
      state.manifest,
      state.presetIndex,
      state.inputs,
      (state.session.completedRuns || 0) + 1,
      state.mode,
    );
    const result = await state.worker.beginStep(request);
    state.session = result.status;
    state.run = null;
    state.step = {
      active: true,
      begin: result.begin,
      reports: [],
      player: defaultWorkbenchPlayer(),
    };
    state.stage = "EXECUTE";
  } catch (error) {
    if (error?.recovery === "REOPEN_REQUIRED" ||
        error?.recovery === "OPEN_REQUIRED") {
      state.session = null;
      state.step = null;
    }
    state.error =
      "Не удалось начать пошаговое исполнение: " + errorText(error);
  }
  state.loading = false;
  render(state);
}

async function stepOnce(state) {
  state.loading = true;
  state.error = null;
  render(state);
  try {
    const result = await state.worker.step();
    state.session = result.status;
    state.step.reports.push(result.step);
    if (result.step.completed) {
      state.step.active = false;
      state.run = completedStepRun(state.step);
      state.stage = "RESULT";
      await history(state);
    } else {
      state.stage = "EXECUTE";
    }
  } catch (error) {
    if (error?.recovery === "REOPEN_REQUIRED" ||
        error?.recovery === "OPEN_REQUIRED") {
      state.session = null;
      state.step = null;
      state.history = null;
    }
    state.error = "Ошибка шага исполнения: " + errorText(error);
  }
  state.loading = false;
  render(state);
}

async function close(state) {
  stopWorkbenchPlayer(state);
  state.loading = true;
  render(state);
  try {
    await state.worker.closeSession();
  } catch (error) {
    state.error = "Ошибка закрытия Worker Session: " + errorText(error);
  }
  state.session = null;
  state.run = null;
  state.step = null;
  state.history = null;
  state.workerProgress = null;
  state.workerRunActive = false;
  state.cancelling = false;
  state.loading = false;
  render(state);
}

function stopWorkbenchPlayer(state) {
  if (state.playerTimer != null) {
    clearTimeout(state.playerTimer);
    state.playerTimer=null;
  }
  if (state.step?.player) state.step.player.playing=false;
}

function setWorkbenchPlayerMode(state, mode) {
  if (!state.step) return;
  stopWorkbenchPlayer(state);
  state.step.player=state.step.player || defaultWorkbenchPlayer();
  state.step.player.mode=mode;
  const frames=workbenchTraceFrames(state.step,mode);
  state.step.player.cursor=frames.length ? frames.length-1 : -1;
  render(state);
}

function moveWorkbenchPlayer(state, delta) {
  if (!state.step) return false;
  stopWorkbenchPlayer(state);
  const snapshot=workbenchPlayerSnapshot(state.step);
  if (!snapshot.frameCount) return false;
  const next=Math.max(0,Math.min(
    snapshot.frameCount-1,
    snapshot.cursor+delta,
  ));
  if (next===snapshot.cursor) return false;
  state.step.player.cursor=next;
  render(state);
  return true;
}

async function advanceWorkbenchPlayer(state) {
  if (!state.step || state.loading) return false;
  const before=workbenchPlayerSnapshot(state.step);
  if (before.hasCachedNext) {
    state.step.player.cursor=before.cursor+1;
    render(state);
    return true;
  }
  if (!state.step.active) return false;

  const mode=state.step.player.mode;
  const oldCount=workbenchTraceFrames(state.step,mode).length;
  await stepOnce(state);
  if (!state.step) return false;
  const frames=workbenchTraceFrames(state.step,mode);
  if (frames.length<=oldCount) return false;
  state.step.player.cursor=oldCount;
  render(state);
  return true;
}

async function finishWorkbenchPlayer(state) {
  if (!state.step) return;
  stopWorkbenchPlayer(state);
  const max=Number(state.step.begin?.maxReactions || 4096);
  let guard=0;
  while (state.step.active && guard<max) {
    await stepOnce(state);
    guard+=1;
    if (state.error) break;
  }
  if (state.step) {
    const frames=workbenchTraceFrames(state.step,state.step.player.mode);
    state.step.player.cursor=frames.length ? frames.length-1 : -1;
  }
  render(state);
}

function scheduleWorkbenchPlayer(state) {
  if (!state.step?.player?.playing) return;
  state.playerTimer=setTimeout(async () => {
    state.playerTimer=null;
    if (!state.step?.player?.playing) return;
    const moved=await advanceWorkbenchPlayer(state);
    if (!moved || !state.step) {
      stopWorkbenchPlayer(state);
      render(state);
      return;
    }
    if (state.step.player.playing) scheduleWorkbenchPlayer(state);
  },state.step.player.speedMs || 800);
}

function playWorkbenchPlayer(state) {
  if (!state.step) return;
  stopWorkbenchPlayer(state);
  state.step.player.playing=true;
  render(state);
  scheduleWorkbenchPlayer(state);
}

function jumpWorkbenchPlayer(state, kind) {
  if (!state.step) return;
  stopWorkbenchPlayer(state);
  if (state.step.player.mode!=="context") {
    state.step.player.mode="context";
  }
  const frames=workbenchTraceFrames(state.step,"context");
  const current=Math.max(-1,state.step.player.cursor);
  let target=frames.findIndex((frame,index) =>
    index>current && frame.kind===kind);
  if (target<0) target=frames.findIndex((frame) => frame.kind===kind);
  if (target>=0) state.step.player.cursor=target;
  render(state);
}

function toggleWorkbenchOverlay(state, name, checked) {
  if (!state.step) return;
  state.step.player.overlays[name]=Boolean(checked);
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
    return '<div class="wb-help">Интерфейс не выдумывает дифференциальный результат. Поддержка сценария и доступность конкретного хоста показаны отдельно.</div>' +
      '<table class="wb-table"><thead><tr><th>Исполнитель</th><th>Поддержка сценарием</th><th>Доступность хоста</th><th>Текущее состояние</th></tr></thead><tbody>' +
      BACKENDS.map(([id, label]) => {
        const availability =
          workbenchBackendAvailability(state.manifest, id);
        return '<tr><td>' + esc(label) + '</td><td>' +
          esc(availability.scenarioSupported ? "заявлена поддержка" : "не заявлена") +
          '</td><td>' +
          esc(availability.sessionHostSupported ? "доступен как Workbench Session" : "недоступен как Workbench Session") +
          '</td><td>' +
          esc(id === state.backend && state.session ? "активная сессия" :
            availability.executable ? "готов к открытию" :
            availability.scenarioSupported ? "другой host" : "не поддерживается") +
          '</td></tr>';
      }).join("") + '</tbody></table>' +
      '<div class="wb-help">WebGPU исполняется отдельным resident witness, LinksDB — native Rust host. Обычный Workbench Session не подменяет их CPU. Дифференциальное сравнение — #281.</div>';
  }
  const recursive = workbenchRecursiveStructure(run);
  return '<div class="wb-help">Уровни ниже независимы: отсутствие данных не считается успешной проверкой.</div>' +
    verificationLevelsHtml(run) +
    freshInstanceEvidenceHtml(run) +
    '<div class="wb-help">Доказательства и сырые структуры по умолчанию свёрнуты.</div>' +
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
  const stepSnapshot = workbenchStepSnapshot(state.step);
  const pipeline = deriveWorkbenchPipeline(status, run, state.step);
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
    '<span class="wb-chip">запуск ' +
      esc(run?.sessionRunId ?? stepSnapshot.sessionRunId ?? 0) +
      '</span>' +
    (state.step?.active
      ? '<span class="wb-chip good">ПОШАГОВО · реакция ' +
        esc(stepSnapshot.reactionCount) + '</span>'
      : '') +
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
    '"' + (state.step?.active ? " disabled" : "") +
    '>Готовый</button><button data-mode="manual" aria-pressed="' + (state.mode === "manual") +
    '"' + (state.step?.active ? " disabled" : "") +
    '>Ручной</button></div>' +

    '<div class="wb-field"><label>Готовый запуск</label><select id="wb-preset-run"' +
    (state.step?.active ? " disabled" : "") + '>' +
    (state.manifest.runSequence || []).map((item, i) =>
      '<option value="' + i + '"' + (i === state.presetIndex ? " selected" : "") + '>' +
      esc("Запуск " + (i + 1) + " · " + (item.runId || ("run-" + (i + 1)))) + '</option>').join("") + '</select></div>' +

    (state.manifest.inputSchema || []).map((field) => {
      const value = state.inputs[field.key];
      const disabled =
        state.mode === "preset" || state.step?.active ? " disabled" : "";
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
      const availability =
        workbenchBackendAvailability(state.manifest, id);
      const suffix = !availability.scenarioSupported
        ? " — не поддерживается сценарием"
        : !availability.sessionHostSupported
          ? " — недоступен как Workbench Session"
          : "";
      return '<option value="' + id + '"' + (id === state.backend ? " selected" : "") +
        (availability.executable ? "" : " disabled") + '>' +
        esc(label + suffix) + '</option>';
    }).join("") + '</select><div class="wb-help">Поддержка сценария и возможности текущего хоста различаются. Скрытого переключения на другой исполнитель нет.</div></div>' +
    gpuWitnessHtml(state) +

    '<div class="wb-actions"><button id="wb-open"' +
      (openSession || state.loading ||
       !workbenchBackendAvailability(state.manifest, state.backend).executable
        ? " disabled" : "") +
    '>Открыть апамять</button><button id="wb-run" class="primary"' +
      (!openSession || state.loading || state.step?.active ? " disabled" : "") +
    '>Выполнить полностью</button><button id="wb-step-start"' +
      (!openSession || state.loading || state.step?.active ? " disabled" : "") +
    '>Начать по шагам</button><button id="wb-cancel"' +
      (!state.workerRunActive ? " disabled" : "") +
    '>' + (state.cancelling ? "Отмена запрошена…" : "Отменить на границе реакции") +
    '</button><button id="wb-close"' +
      (!openSession || state.loading ? " disabled" : "") +
    '>Закрыть</button></div>' +
    '<div class="wb-help">Готовый и ручной режим используют один и тот же манифест сценария. Изменение входов сохраняет эту же сессию и уже загруженную апамять.</div>' +
    (state.error ? '<p class="wb-error">' + esc(state.error) + '</p>' : '') + '</div>' +

    '<div class="wb-card wb-main"><h3>Одна постоянная апамять</h3>' +
    '<div class="wb-simple-grid">' +
    '<div class="wb-simple-item"><small>Загружено</small><strong>' +
      esc(state.manifest.title || state.manifest.scenarioId || state.manifest.programProfileId || "Сценарий") +
    '</strong></div>' +
    '<div class="wb-simple-item"><small>Входы</small><code>' + esc(JSON.stringify(state.inputs)) + '</code></div>' +
    '<div class="wb-simple-item"><small>Текущее состояние</small><strong>' +
      esc(!status
        ? "апамять закрыта"
        : state.workerRunActive
          ? "Worker · завершено реакций " +
            (state.workerProgress?.completedSteps || 0)
          : state.step?.active
            ? "пошаговое исполнение · реакция " + stepSnapshot.reactionCount
            : run
              ? "запуск №" + run.sessionRunId + " завершён"
              : "загружено один раз · готово к выполнению") +
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
    (state.step ? workbenchPlayerHtml(state.step) : '') +
    (state.step
      ? '<div class="wb-help">Scope показан только из runtime ReactionEvidence. UI не передаёт Scope обратно в исполнитель.</div>'
      : '') +
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
    void (async () => {
      try {
        await loadManifest(state, Number(event.target.value));
      } catch (error) {
        state.error =
          "Не удалось загрузить сценарий: " + errorText(error);
      }
      render(state);
    })();
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
  root.querySelector("#wb-gpu-witness")?.addEventListener("click", () => {
    void runGpuWitness(state);
  });
  for (const input of root.querySelectorAll("[data-input-key]")) {
    input.addEventListener("change", () => { state.inputs[input.dataset.inputKey] = input.value; });
  }
  root.querySelector("#wb-open")?.addEventListener("click", () => void open(state));
  root.querySelector("#wb-run")?.addEventListener("click", () => {
    syncInputs(state);
    state.step = null;
    void execute(state);
  });
  root.querySelector("#wb-step-start")?.addEventListener("click", () => {
    syncInputs(state);
    void beginStep(state);
  });
  root.querySelector("#wb-step-next")?.addEventListener("click", () => {
    void advanceWorkbenchPlayer(state);
  });
  root.querySelector("#wb-player-prev")?.addEventListener("click", () => {
    moveWorkbenchPlayer(state,-1);
  });
  root.querySelector("#wb-player-play")?.addEventListener("click", () => {
    playWorkbenchPlayer(state);
  });
  root.querySelector("#wb-player-pause")?.addEventListener("click", () => {
    stopWorkbenchPlayer(state); render(state);
  });
  root.querySelector("#wb-player-end")?.addEventListener("click", () => {
    void finishWorkbenchPlayer(state);
  });
  root.querySelector("#wb-jump-create")?.addEventListener("click", () => {
    jumpWorkbenchPlayer(state,"CONTEXT_CREATED");
  });
  root.querySelector("#wb-jump-collapse")?.addEventListener("click", () => {
    jumpWorkbenchPlayer(state,"CONTEXT_COLLAPSED");
  });
  root.querySelector("#wb-player-speed")?.addEventListener("change", (event) => {
    if (state.step?.player) {
      state.step.player.speedMs=Number(event.target.value);
      if (state.step.player.playing) {
        stopWorkbenchPlayer(state);
        playWorkbenchPlayer(state);
      } else render(state);
    }
  });
  for (const button of root.querySelectorAll("[data-player-mode]")) {
    button.addEventListener("click", () => {
      setWorkbenchPlayerMode(state,button.dataset.playerMode);
    });
  }
  for (const input of root.querySelectorAll("[data-overlay]")) {
    input.addEventListener("change", () => {
      toggleWorkbenchOverlay(state,input.dataset.overlay,input.checked);
    });
  }
  root.querySelector("#wb-cancel")?.addEventListener("click", () => {
    void cancelRun(state);
  });
  root.querySelector("#wb-close")?.addEventListener("click", () => {
    void close(state);
  });
  for (const button of root.querySelectorAll("[data-tab]")) {
    button.addEventListener("click", () => { state.tab = button.dataset.tab; render(state); });
  }
}

export async function mountWorkbench(root) {
  styles();
  const state = {
    root,
    catalogWasm: null,
    worker: null,
    registry: { entries: [] },
    manifest: { runSequence: [], inputSchema: [] },
    scenarioIndex: 0, presetIndex: 0, inputs: {}, mode: "preset",
    backend: "optimized-cpu",
    session: null, run: null, step: null, history: null, tab: "timeline",
    gpuWitness: { status: "idle" },
    workerRunActive: false, workerProgress: null, cancelling: false,
    playerTimer: null,
    level: "simple", stage: "RESULT",
    build: { version: "загрузка", sha: "загрузка" }, loading: true, error: null,
  };
  root.innerHTML = '<div class="notice">Загрузка реальной рабочей лаборатории апамяти…</div>';
  try {
    const loaded = await Promise.all([catalogWasm(), buildInfo()]);
    state.catalogWasm = loaded[0];
    state.build = loaded[1];
    state.worker = new ScenarioWorkerClient();
    await state.worker.init();
    state.registry = refreshScenarioPresetRegistry(state.catalogWasm);
    let index = state.registry.entries.findIndex(
      (entry) => entry.scenarioId === "mux1-lifecycle"
    );
    if (index < 0) index = 0;
    await loadManifest(state, index);
    // Expose the mounted state on its own root for deterministic browser E2E
    // and diagnostics without creating a second execution surface.
    root.__amemoryWorkbenchState = state;
    state.loading = false;
    render(state);
    await open(state);
    if (typeof location !== "undefined" &&
        new URLSearchParams(location.search).get("webgpuWitness") === "1") {
      await runGpuWitness(state);
    }
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
