# amemory

Библиотека реализаций и симуляций **A-memory**.

## Назначение

`amemory` не задаёт семантику MTS. Канонический человекочитаемый смысловой владелец находится в `netkeep80/anum_docs`:

- `docs/specs/Апамять и управление сетью связей.md`.

Для независимых реализаций полной апамяти дополнительно принят отдельный машинный профиль исполнения:

```text
anum_docs/profiles/amemory-execution-profile.json

schema         = mts-amemory-execution-profile/v0.1
id             = minimal-portable-amemory-execution
profileVersion = 0.1.0
```

JSON-профиль является машинной проекцией канонического документа, а не вторым смысловым владельцем.

## Два независимых upstream pin

`amemory` фиксирует отдельно:

```text
accepted MTS v0.13 foundation
  commit = 440caf09558d4ff5cfda11805cb3ef97b48d1ad5

minimal-portable-amemory-execution v0.1.0
  commit = af4e3dadbb9857fba7239a58f79ed2da59bc5c42
  blob   = e2c614e1556f31f6230cb69d686d1f22ab2c97e8
```

Это принципиальная граница: обновление execution profile не должно незаметно менять принятую MTS v0.13, а обновление foundation не должно молча менять профиль исполнения.

Локальные machine-readable источники:

```text
contracts/upstream/mts-v0.13.lock.json
contracts/upstream/mts-v0.13-requirements.json
```

Второй файл генерируется детерминированно из двух exact upstream pin и не редактируется вручную как источник семантики.

## Главный принцип

```text
accepted MTS v0.13
        +
minimal-portable-amemory-execution
        |
        +--> reference CPU / Rust / WASM
        +--> optimized CPU
        +--> Links/Doublets
        +--> WebGPU / accelerator
        +--> future specialized hardware
```

Внутреннее устройство реализаций **не обязано совпадать**.

Не являются семантикой MTS или execution profile сами по себе:

```text
GPU layout
thread order
work-group order
GPU lane order
local handle
allocation order
host cursor representation
Memory / Doublets API shape
```

Эти вещи могут быть частью субстрата реализации, но не смысловым полномочием.

## D7: обязательная декларация backend

Каждая реализация должна явно объявлять:

```text
backendId
supportedMtsVersion
implementedProfileId
implementedProfileVersion
profilePinCommit
implementationStatus
supportLevel
declaredScope
currentScopeMechanism
oldPhysicalLinkRetentionPolicy
backendSubstrateBoundary
nonSemanticSchedulingChoices
normalizedSemanticEquivalence
differentialEvidence
```

Машинная матрица находится в:

```text
contracts/amemory-conformance-v0.1.json#/backendMatrix
```

### Текущее состояние backend

| Backend | Статус | Current Scope | Физическая история | Нормализованное сравнение | Полный профиль |
|---|---|---|---|---|---|
| Rust reference CPU / WASM | prototype / partial | R1–R5: bounded two-bank Scope + published selector | старые Scope bank и Links сохраняются физически | canonical Anum reaction Scope + matched/handoff | **нет — audit #45: P06/P08/P15 ждут R6** |
| WebGPU browser | prototype / partial | R1–R5: two-bank Scope buffer + atomic published selector | старые GPU Scope bank и Link pool сохраняются | canonical Anum reaction Scope + matched/handoff | **нет — audit #45: P06/P08/P15 ждут R6** |
| future accelerator family | planned | должен быть объявлен до реализации | должен быть объявлен отдельно от semantic currentness | обязательный differential против reference | **нет** |

Rust native tests и WASM browser используют одну текущую reference-кодовую базу; это разные поверхности исполнения одного prototype backend, а не разные семантики.

## Что уже доказано браузерным prototype

Текущие GREEN witnesses подтверждают полезные части будущего профиля:

- CPU/WebGPU incidence differential;
- обнаружение намеренного mismatch;
- canonical pair convergence;
- stateful физическое хранение между rounds;
- независимость portable identity от local handles;
- canonical Anum import/export в двух независимо адресованных Memories;
- fail-closed malformed/foreign/capacity controls.

Но это **не равно полной реализации реакции A-сети**.

Текущая классификация после глобального evidence-аудита #45:

```text
R1 fixture:
  K = 98
  A = 68
  B = 16898

  K ⟼ A = 19868
  A ⟼ B = 16816898
  K ⟼ B = 19816898

R1 executable subset:
  P01 P02 P03 P04 P05 P11 P12
    -> GREEN on real browser CPU/WASM ↔ WebGPU differential

Audit #45:
  P06 = exhaustive admitted relations     -> нужен direct MANY witness
  P08 = replace by all outputs            -> нужен direct MANY witness
  P15 = schedule/order non-semantic       -> нужен reaction-order permutation witness

Observed R1:
  before CPU/GPU = [19868]
  after  CPU/GPU = [19816898]
  matchedRelations = 1 / 1
  handoffCount = 1 / 1
  TheorySnapshot isolation = PASS
  old Scope retained physically = PASS
  backend-local handles differ = PASS
  negative mismatch/no-match/foreign controls = PASS

R2 executable subset:
  P07 P13
    -> GREEN on real browser CPU/WASM ↔ WebGPU differential

Observed R2:
  Scope CPU/GPU = [19868]
  matchedRelations = 0 / 0
  handoffCount = 0 / 0
  quiescent = true / true
  published Scope unchanged = PASS
  runtime failure != quiescence = PASS

R3 executable subset:
  P09 P10
    -> GREEN on real browser CPU/WASM ↔ WebGPU differential

Observed R3 ZERO:
  Scope CPU/GPU = []
  matchedRelations = 1 / 1
  handoffCount = 1 / 1
  quiescent = false / false
  old Scope retained physically = PASS

Observed R3 mixed ZERO + duplicate convergence:
  Scope CPU/GPU = [19816898]
  matchedRelations = 3 / 3
  handoffCount = 1 / 1
  quiescent = false / false
  duplicate convergence = PASS (single canonical successor)

R4 executable subset:
  P14
    -> GREEN on real browser CPU/WASM ↔ WebGPU trajectory differential

Observed R4:
  snapshot_t = [A⟼B]
  live Theory after admission = [A⟼B, B⟼C]
  reaction t: K⟼A -> K⟼B
  same-reaction visibility of B⟼C = false
  snapshot_t+1 sees both relations
  reaction t+1: K⟼B -> K⟼C
  stale-snapshot control = PASS
  normalized CPU/GPU trajectory = PASS

R5 executable subset:
  P16 P17
    -> GREEN on real browser CPU/WASM ↔ WebGPU trajectory differential

Observed R5:
  S0 = [199898]
  S1 = [199868]  // K⟼END
  S2 = [199898]
  S3 = [199868]
  S4 = [199898]
  matchedRelations = 1 on every step
  handoffCount = 1 on every step
  quiescent = false on every step
  recurrence = PASS
  structural END continuation = PASS
  bounded witness returned = PASS

AM-C046 R1 = GREEN
AM-C047 R2 = GREEN
AM-C048 R3 = GREEN
AM-C049 R4 = GREEN
AM-C050 R5 = GREEN
AM-C045 FULL PROFILE = PENDING R6 DIRECT WITNESSES
FULL_REACTION_PROFILE_CONFORMANCE = FALSE
```

Для двух объявленных prototype-backends — Rust/CPU/WASM и browser WebGPU — R1–R5 дают сильное реальное browser differential evidence, но глобальный аудит #45 обнаружил три закона, которым нужен отдельный прямой witness: `P06`, `P08`, `P15`. До R6 полный профиль не заявляется.

Это не означает, что весь репозиторий принят целиком: repository-wide `acceptanceState` остаётся независимым и может сохранять `NOT_READY` из-за других planned conformance vectors.

Реализация продолжает потреблять machine profile, а не копировать смысл из control flow `anum_docs/ts/src/v013-grounded-execution.ts`.

## Conformance

Статус полного backend-conformance допустим только после прохождения общего corpus для точной пары:

```text
accepted MTS foundation revision
+
execution profile revision
```

Экспериментальный backend может поддерживать часть поведения, но обязан:

- fail closed вне заявленного scope;
- не выдавать supporting storage evidence за reaction conformance;
- показывать фактический execution path;
- сравниваться через нормализованное переносимое наблюдение;
- иметь отрицательный differential control.

Производительность никогда не является доказательством корректности семантики.

## Направления

1. **Reference CPU / Rust / WASM** — простой проверяемый oracle.
2. **Optimized CPU backend** — специализированная реализация апамяти.
3. **Links/Doublets backend** — отдельный substrate experiment; его полная корректность дополнительно зависит от `anum_docs#1332`.
4. **WebGPU / associative accelerator** — физически отличная accelerator-реализация того же execution profile.
5. В дальнейшем — другие ускорители и специализированное железо.

## Граница репозиториев

```text
anum_docs
  MTS semantic authority
  +
  portable A-memory execution profile
        |
        v
amemory
  backend/substrate implementations
  +
  equivalence evidence / falsifiers
        |
        v
aprover
  consumer
```

Если реализация обнаруживает новый семантический пробел, он возвращается в `anum_docs`; implementation consensus не определяет MTS truth.

## Независимая Doublets-граница

`anum_docs#1332` остаётся отдельным исследованием отображения:

```text
MTS semantic Link identity
        <->
Doublets element / Aset representation
```

Она блокирует полный conformant claim именно для Links/Doublets backend, но не reference CPU/WASM или WebGPU execution research.
