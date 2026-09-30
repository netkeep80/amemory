import {
  beginScenarioLiveStepRun,
  closeScenarioLiveSession,
  openScenarioLiveSession,
  readScenarioTransportLimits,
  refreshScenarioLiveHistoryStatus,
  runScenarioLiveSession,
  setScenarioLiveRetentionPolicy,
  stepScenarioLiveSession,
} from "./scenario-transport.mjs";
import {
  loadScenarioPresetManifestByIndex,
  refreshScenarioPresetRegistry,
} from "./scenario-presets.mjs";
import { collectBrowserProof } from "./i386-proof-transport.mjs";
import { readJsonAbi } from "./i386-wasm-json.mjs";
import { recursiveStructureHtml } from "./proof-view.mjs";
import {
  PROOF_VERIFICATION_PROFILE_ID,
  verifyCompactExecutionProof,
} from "./proof-verifier.mjs";
import {
  verifyLiveStepTrace,
  verifyLiveTraceAgainstCompactProof,
} from "./runtime-trace-verifier.mjs";

const BACKENDS = [
  ["optimized-cpu", "CPU · оптимизированный"],
  ["webgpu", "WebGPU"],
  ["linksdb", "LinksDB"],
];

const LAB_RESULT_ABI = Object.freeze({
  available: "amemory_i386_lab_result_available",
  length: "amemory_i386_lab_result_json_len",
  pointer: "amemory_i386_lab_result_json_ptr",
  byte: "amemory_i386_lab_result_json_byte",
});

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


export function workbenchEvidenceReports(trace, finalResult) {
  if (!Array.isArray(trace) || trace.length === 0) {
    throw new Error("Пустая трасса evidence");
  }
  let activeReactionCount = 0;
  return trace.map((rawEvidence, index) => {
    const evidence = clone(rawEvidence);
    if (evidence.quiescent !== true) activeReactionCount += 1;
    const completed =
      index === trace.length - 1 && evidence.quiescent === true;
    return {
      sessionRunId: evidence.runId,
      activeReactionCount,
      completed,
      evidence,
      result: completed ? clone(finalResult) : null,
      assertionResults: [],
    };
  });
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
    "amemory_scenario_live_step_begin_json",
    "amemory_scenario_live_step_json",
    "amemory_scenario_live_close",
    "amemory_i386_memory_run",
    "amemory_i386_lab_result_available",
    "amemory_i386_lab_result_json_len",
    "amemory_i386_lab_result_json_byte",
    "amemory_i386_lab_compact_proof_available",
    "amemory_i386_lab_compact_proof_json_len",
    "amemory_i386_lab_compact_proof_json_byte",
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
  stopWorkbenchPlayer(state);
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
  state.step = null;
  state.m6Replay = null;
  state.history = null;
  state.error = null;
  presetInputs(state);
}

async function history(state) {
  const refreshed = refreshScenarioLiveHistoryStatus(state.wasm);
  state.history = refreshed.ok ? refreshed.status : null;
}

async function open(state) {
  stopWorkbenchPlayer(state);
  state.loading = true; state.error = null; render(state);
  try {
    const opened = openScenarioLiveSession(state.wasm, state.manifest, state.backend);
    if (!opened.ok) throw new Error(opened.error && opened.error.message || JSON.stringify(opened.error));
    state.session = opened.status;
    state.run = null;
    state.step = null;
    state.m6Replay = null;
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
  stopWorkbenchPlayer(state);
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
    state.m6Replay = null;
    await history(state);
  } catch (error) {
    state.error = "Ошибка исполнения: " + errorText(error);
  }
  state.loading = false; render(state);
}

async function beginStep(state) {
  state.loading = true; state.error = null; render(state);
  try {
    const request = createWorkbenchStepRun(
      state.manifest,
      state.presetIndex,
      state.inputs,
      (state.session.completedRuns || 0) + 1,
      state.mode,
    );
    const result = beginScenarioLiveStepRun(state.wasm, request);
    if (!result.ok) {
      throw new Error(result.error?.message || JSON.stringify(result.error));
    }
    state.session = result.payload.status;
    state.run = null;
    state.step = {
      active: true,
      begin: result.payload.begin,
      reports: [],
      player: defaultWorkbenchPlayer(),
    };
    state.stage = "EXECUTE";
  } catch (error) {
    state.error = "Не удалось начать пошаговое исполнение: " +
      errorText(error);
  }
  state.loading = false; render(state);
}

async function stepOnce(state) {
  state.loading = true; state.error = null; render(state);
  try {
    const result = stepScenarioLiveSession(state.wasm);
    if (!result.ok) {
      throw new Error(result.error?.message || JSON.stringify(result.error));
    }
    state.session = result.payload.status;
    state.step.reports.push(result.payload.step);
    if (result.payload.step.completed) {
      state.step.active = false;
      state.run = completedStepRun(state.step);
      state.stage = "RESULT";
      await history(state);
    } else {
      state.stage = "EXECUTE";
    }
  } catch (error) {
    state.error = "Ошибка шага исполнения: " + errorText(error);
  }
  state.loading = false; render(state);
}


async function runM6EvidenceReplay(state) {
  stopWorkbenchPlayer(state);
  state.loading = true;
  state.error = null;
  render(state);
  try {
    if (state.session) {
      closeScenarioLiveSession(state.wasm);
      state.session = null;
      state.history = null;
    }

    if (state.wasm.amemory_i386_memory_run(0x25, 0xab) !== 1) {
      throw new Error("реальный M6A_RADIX_PAGE запуск отклонён");
    }

    const envelope = readJsonAbi(
      state.wasm,
      LAB_RESULT_ABI,
      "M6A witness result",
    );
    const payload = envelope?.payload;
    if (envelope?.witnessKind !== "memory-radix" ||
        payload?.block !== "M6A_RADIX_PAGE" ||
        payload?.traceSource !== "ProofRuntimeSession.step(TRACE)" ||
        !Array.isArray(payload?.trace) ||
        payload.trace.length === 0) {
      throw new Error("M6A не вернул каноническую runtime trace");
    }

    const collected = collectBrowserProof(state.wasm);
    const compactProof = collected.compactProof;
    if (!compactProof) {
      throw new Error("M6A не вернул compact-proof");
    }

    const finalResult = {
      block: payload.block,
      value: Number(payload.after) >>> 0,
      compactProofValue: Number(compactProof.result?.decodedValue) >>> 0,
    };
    const reports = workbenchEvidenceReports(payload.trace, finalResult);
    const liveVerification = verifyLiveStepTrace(reports);
    const crossVerification =
      verifyLiveTraceAgainstCompactProof(reports, compactProof);
    const compactVerification =
      verifyCompactExecutionProof(compactProof);

    if (liveVerification.traceConsistent !== true ||
        liveVerification.completed !== true ||
        crossVerification.compactCrossConsistent !== true ||
        compactVerification.semanticReplayVerified !== true ||
        finalResult.value !== finalResult.compactProofValue) {
      throw new Error(
        "M6A trace/result не прошли независимую проверку против compact-proof",
      );
    }

    state.run = null;
    state.m6Replay = {
      payload,
      compactProof,
      liveVerification,
      crossVerification,
      compactVerification,
    };
    state.step = {
      active: false,
      begin: {
        sessionRunId: reports[0].sessionRunId,
        evidenceSessionId: reports[0].evidence.sessionId,
        sourceLabel: "M6A_RADIX_PAGE · real TRACE",
        linksBeforeConfigure: reports[0].evidence.linksBefore,
        linksAfterConfigure: reports[0].evidence.linksBefore,
      },
      reports,
      player: {
        ...defaultWorkbenchPlayer(),
        mode: "reaction",
        cursor: 0,
      },
    };
    state.level = "proof";
    state.stage = "EVIDENCE";
    state.tab = "proof";
  } catch (error) {
    state.step = null;
    state.m6Replay = null;
    state.error = "Не удалось воспроизвести M6A: " + errorText(error);
  }
  state.loading = false;
  render(state);
}

function close(state) {
  stopWorkbenchPlayer(state);
  if (state.session) closeScenarioLiveSession(state.wasm);
  state.session = null;
  state.run = null;
  state.step = null;
  state.m6Replay = null;
  state.history = null;
  state.error = null;
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
    '<span class="wb-chip">сессия ' +
      esc(compact(
        status?.sessionId ??
        state.step?.begin?.evidenceSessionId ??
        null
      )) + '</span>' +
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
    (openSession || state.m6Replay ? " disabled" : "") + '>' +
    (state.registry.entries || []).map((entry, i) =>
      '<option value="' + i + '"' + (i === state.scenarioIndex ? " selected" : "") + '>' +
      esc((entry.title || ("Сценарий " + entry.scenarioId)) +
      (entry.scenarioVersion ? " @" + entry.scenarioVersion : "")) + '</option>').join("") +
    '</select><div class="wb-help">' + esc(state.manifest.description || "") + '</div></div>' +

    '<div class="wb-mode"><button data-mode="preset" aria-pressed="' + (state.mode === "preset") +
    '"' + (state.step?.active || state.m6Replay ? " disabled" : "") +
    '>Готовый</button><button data-mode="manual" aria-pressed="' + (state.mode === "manual") +
    '"' + (state.step?.active || state.m6Replay ? " disabled" : "") +
    '>Ручной</button></div>' +

    '<div class="wb-field"><label>Готовый запуск</label><select id="wb-preset-run"' +
    (state.step?.active || state.m6Replay ? " disabled" : "") + '>' +
    (state.manifest.runSequence || []).map((item, i) =>
      '<option value="' + i + '"' + (i === state.presetIndex ? " selected" : "") + '>' +
      esc("Запуск " + (i + 1) + " · " + (item.runId || ("run-" + (i + 1)))) + '</option>').join("") + '</select></div>' +

    (state.manifest.inputSchema || []).map((field) => {
      const value = state.inputs[field.key];
      const disabled =
        state.mode === "preset" || state.step?.active || state.m6Replay ? " disabled" : "";
      const control = String(field.type || "").toUpperCase() === "BIT"
        ? '<select data-input-key="' + esc(field.key) + '"' + disabled + '><option value="0"' +
          (Number(value) === 0 ? " selected" : "") + '>0</option><option value="1"' +
          (Number(value) === 1 ? " selected" : "") + '>1</option></select>'
        : '<input data-input-key="' + esc(field.key) + '" value="' + esc(value) + '"' + disabled + '>';
      return '<div class="wb-field"><label>' + esc(field.key) + " · " + esc(field.type) +
        '</label>' + control + '<div class="wb-help">' + esc(field.description || "") + '</div></div>';
    }).join("") +

    '<div class="wb-field"><label>Исполнитель</label><select id="wb-backend"' + (openSession || state.m6Replay ? " disabled" : "") + '>' +
    BACKENDS.map(([id, label]) => {
      const supported = (state.manifest.supportedИсполнительs || []).includes(id);
      return '<option value="' + id + '"' + (id === state.backend ? " selected" : "") +
        (supported ? "" : " disabled") + '>' + esc(label + (supported ? "" : " — не поддерживается")) + '</option>';
    }).join("") + '</select><div class="wb-help">Неподдерживаемые исполнители показаны явно; скрытого переключения на другой исполнитель нет.</div></div>' +

    '<div class="wb-actions"><button id="wb-open"' +
      (openSession || state.loading ? " disabled" : "") +
    '>Открыть апамять</button><button id="wb-run" class="primary"' +
      (!openSession || state.loading || state.step?.active ? " disabled" : "") +
    '>Выполнить полностью</button><button id="wb-step-start"' +
      (!openSession || state.loading || state.step?.active ? " disabled" : "") +
    '>Начать по шагам</button><button id="wb-m6-replay"' +
      (state.loading || state.step?.active ? " disabled" : "") +
    '>M6A · реальная память по шагам</button><button id="wb-close"' +
      ((!openSession && !state.m6Replay) || state.loading ? " disabled" : "") +
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
      esc(!status && state.m6Replay
        ? "M6A evidence · " + stepSnapshot.reactionCount +
          " реальных реакций в одной A-memory"
        : !status
          ? "апамять закрыта"
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
    (state.m6Replay
      ? '<div class="wb-help"><strong>M6A проверен:</strong> ' +
        'TRACE_CONSISTENT · COMPACT_CROSS_CONSISTENT · ' +
        'SEMANTIC_REPLAY_VERIFIED · final value=' +
        esc(state.m6Replay.payload.after) + ' = compact decodedValue=' +
        esc(state.m6Replay.compactProof.result.decodedValue) + '</div>'
      : '') +
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
  root.querySelector("#wb-run")?.addEventListener("click", () => {
    syncInputs(state);
    state.step = null;
    void execute(state);
  });
  root.querySelector("#wb-step-start")?.addEventListener("click", () => {
    syncInputs(state);
    void beginStep(state);
  });
  root.querySelector("#wb-m6-replay")?.addEventListener("click", () => {
    void runM6EvidenceReplay(state);
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
    session: null, run: null, step: null, m6Replay: null,
    history: null, tab: "timeline", playerTimer: null,
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
