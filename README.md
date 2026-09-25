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
| Rust reference CPU / WASM | prototype / partial | R1: bounded two-bank Scope + published selector | old R1 Scope bank и Links сохраняются физически | canonical Anum reaction Scope + matched/handoff | **нет** |
| WebGPU browser | prototype / partial | R1: two-bank Scope buffer + atomic published selector | old R1 GPU Scope bank и Link pool сохраняются | canonical Anum reaction Scope + matched/handoff | **нет** |
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

Текущая классификация после реализации bounded R1:

```text
R1 fixture:
  K = 98
  A = 68
  B = 16898

  K ⟼ A = 19868
  A ⟼ B = 16816898
  K ⟼ B = 19816898

R1 executable subset:
  P01 P02 P03 P04 P05 P06 P08 P11 P12 P15
    -> GREEN on real browser CPU/WASM ↔ WebGPU differential

Observed R1:
  before CPU/GPU = [19868]
  after  CPU/GPU = [19816898]
  matchedRelations = 1 / 1
  handoffCount = 1 / 1
  TheorySnapshot isolation = PASS
  old Scope retained physically = PASS
  backend-local handles differ = PASS
  negative mismatch/no-match/foreign controls = PASS

P10
  -> supporting canonicalization evidence only

P07 P09 P13 P14 P16 P17
  -> not-yet-executed

AM-C046 R1 = GREEN
AM-C045 FULL PROFILE = PLANNED
FULL_REACTION_PROFILE_CONFORMANCE = FALSE
```

R1 использует explicit published Scope и reaction-start `TheorySnapshot`, а CPU/WASM и WebGPU сравниваются по canonical Anum, не по локальным handles.

Storage/incidence/Anum tests и даже R1 нельзя использовать для объявления, что profile `P01-P17` уже реализован целиком.

## Следующие reaction slices

После закрытого GREEN R1 следующий этап — последовательно закрыть:

```text
P07  NO_ADMITTED_RELATION
P09  ZERO / mixed ZERO
P10  duplicate convergence inside reaction
P13  quiescence
P14  Theory admission visibility t+1
P16  recurrence / nontermination
P17  structural END != halt
```

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
