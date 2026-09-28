# amemory

> **Live 80386 A-memory lab:** https://netkeep80.github.io/amemory/  
> Интерактивные реальные A-Circuit/WASM тесты: настраиваемые входы → структурное исполнение в апамяти → результат, флаги и evidence.  
> Версия `amemory`: `VERSION` (SemVer, контролируется repo-guard).

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
accepted MTS v0.14 foundation
  commit = fcbc97e2279471c2c5effed57685c5f49ec856be

minimal-portable-amemory-execution v0.1.0
  commit = af4e3dadbb9857fba7239a58f79ed2da59bc5c42
  blob   = e2c614e1556f31f6230cb69d686d1f22ab2c97e8
  foundationMtsVersion = v0.13
```

Это принципиальная граница: accepted foundation уже MTS v0.14, но execution profile 0.1.0 остаётся независимо pinned post-v0.13 profile. Обновление foundation не должно молча переименовывать или менять профиль исполнения.

Локальные machine-readable источники:

```text
contracts/upstream/mts-v0.14.lock.json
contracts/upstream/mts-v0.14-requirements.json

historical retained pin/projection:
contracts/upstream/mts-v0.13.lock.json
contracts/upstream/mts-v0.13-requirements.json
```

Второй файл генерируется детерминированно из двух exact upstream pin и не редактируется вручную как источник семантики.

## Главный принцип

```text
accepted MTS v0.14
        +
minimal-portable-amemory-execution@0.1.0
(profile foundation remains v0.13)
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
contracts/amemory-conformance-v0.2.json#/backendMatrix
```

### Текущее состояние backend

| Backend | Статус | Current Scope | Физическая история | Нормализованное сравнение | Полный профиль |
|---|---|---|---|---|---|
| Rust reference CPU / WASM | prototype / partial | R1–R7: bounded two-bank Scope + published selector | старые Scope bank и Links сохраняются физически | normalized recursive Link wire Scope + matched/handoff | **да — P01–P17** |
| WebGPU browser | prototype / partial | R1–R7: two-bank Scope buffer + atomic published selector | старые GPU Scope bank и Link pool сохраняются | normalized recursive Link wire Scope + matched/handoff | **да — P01–P17** |
| future accelerator family | planned | должен быть объявлен до реализации | должен быть объявлен отдельно от semantic currentness | обязательный differential против reference | **нет** |

Rust native tests и WASM browser используют одну текущую reference-кодовую базу; это разные поверхности исполнения одного prototype backend, а не разные семантики.

## Что уже доказано браузерным prototype

Текущие GREEN witnesses подтверждают полезные части будущего профиля:

- CPU/WebGPU incidence differential;
- обнаружение намеренного mismatch;
- canonical pair convergence;
- stateful физическое хранение между rounds;
- независимость portable identity от local handles;
- canonical direct technical recursive Link wire import/export в двух независимо адресованных Memories;
- fail-closed malformed/foreign/capacity controls.

### Терминология представлений после MTS v0.14

Каноническая публичная поверхность принятого MTS v0.14 разделяет онтологию, рекурсивную структуру и представления:

```text
онтология:
  Link

рекурсивный структурный алфавит:
  ROOT     8     остенсивно: ∞
  START_K  9S    остенсивно: ♂S
  END_K    6S    остенсивно: S♀
  PAIR     1AB   остенсивно: A ⟼ B

самоинцидентность:
  ROOT  = 11
  START_K = 10
  END_K   = 01
  PAIR  = 00
```

`START_K` и `END_K` — **контекстно-относительные роли**. Они появляются только после ориентации Context `K`; до выбора локальной рамки нет абсолютных глобальных START/END. Знаки `♂/♀` на публичных поверхностях означают именно эти локальные роли.

`8/9/6/1` — рекурсивный структурный алфавит Link, а не алфавит Anum/Q. `∞/♂/♀/⟼` — остенсивная запись той же структурной формы, а не четыре физических opcode.

Наследованные имена `R/O/C/L/U` и примеры `98/68/19868/16898` остаются допустимыми как зафиксированная v0.13 basis / recursive-prefix запись. Они **не являются** Foundation-global authority для абсолютного START/END в v0.14.


Для текущих structural/reaction witnesses используется явное разделение:

```text
recursive Link wire
  !=
Anum / ExactSequence
```

`recursive Link wire` — рекурсивная запись структуры Link для тех поверхностей, где выбран этот codec. Она не объявляет произвольный rooted Link значением Anum.

`Anum / ExactSequence` — отдельная sequence-specific линия представления. Поэтому `resultSequenceAnum` остаётся Anum-полем только там, где источник результата действительно прошёл через проверенную ExactSequence/sequence representation.

Исторические имена `anum-boundary.mjs`, `amemory_anum_cpu_*`, `parseAnum`, `cpuImportRaw` и родственные API сохраняются как **compatibility naming** для direct technical recursive-wire materialization. Они не являются семантическим утверждением `recursive Link wire == Anum`.

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

Audit #45 + real browser R6:
  P06 = exhaustive admitted relations     -> GREEN (1→N distinct MANY)
  P08 = replace by all outputs            -> GREEN (1→N / N→M)
  P15 = schedule/order non-semantic       -> GREEN (reversed current + Theory order)

R6 real browser CPU/WASM ↔ WebGPU:
  1→N distinct MANY                       -> PASS
  N→M                                     -> PASS
  reversed current/Theory order           -> PASS
  bounded-scope fail-closed               -> PASS
  normalized CPU/GPU differential         -> PASS

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
  S1 = [199868]  // context-relative K⟼END_K / ♀ continuation
  S2 = [199898]
  S3 = [199868]
  S4 = [199898]
  matchedRelations = 1 on every step
  handoffCount = 1 on every step
  quiescent = false on every step
  recurrence = PASS
  structural context-relative END_K / ♀ continuation = PASS
  bounded witness returned = PASS

AM-C046 R1 = GREEN
AM-C047 R2 = GREEN
AM-C048 R3 = GREEN
AM-C049 R4 = GREEN
AM-C050 R5 = GREEN
AM-C045 FULL PROFILE = GREEN
FULL_REACTION_PROFILE_CONFORMANCE = TRUE
```

Для двух объявленных prototype-backends — Rust/CPU/WASM и browser WebGPU — реальный browser differential chain R1–R6 напрямую покрывает весь independently pinned `minimal-portable-amemory-execution@0.1.0`: `P01–P17`. R7 отдельно доказывает принятый v0.14 reaction-result basis (`N→{}`, активный `A→A`, `A→A♀`, `A→♂A`) без нового semantic opcode/kernel.

Текущая downstream-линия — `amemory-contract/v0.2` + `amemory-conformance/v0.2`: все mandatory vectors GREEN, `V14-L1..V14-L14` имеют явный downstream disposition, поэтому repository-wide `acceptanceState = READY`. Сам контракт остаётся `candidate / accepted=false` до отдельного явного решения о принятии.

Реализация продолжает потреблять независимо pinned machine execution profile; accepted v0.14 foundation и profile 0.1.0 не смешиваются в одну версию.

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
