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

Текущий пользовательский путь — **Workbench → browser Worker → A-Circuit WASM → persistent optimized-CPU Session**.

| Backend / поверхность | Текущий статус | Что реально поддерживается | Чего пока нет |
|---|---|---|---|
| Optimized CPU | **основной исполняющий backend** | persistent `MemoryInstance/Session`, PREPARE/LOAD один раз, повторные CONFIGURE/RUN, `step()`, бюджеты/rollback/cancel/watchdog, evidence/profile | не является semantic authority MTS |
| Rust reference CPU / WASM | независимый reference/differential backend | explicit `ReferenceMemoryInstance`, изолированные instances, structural differential witnesses | не является текущим Workbench runtime |
| WebGPU | **bounded research capability** | packed-carrier/lookup и накопленные differential witnesses | authentic current MUX1 `discover -> publish` slice #266 ещё не закрыт; persistent Session #279 отсутствует |
| LinksDB / Doublets | planned / experimental | substrate research | persistent Session adapter #280 отсутствует |
| future accelerator | planned | только через общий backend/session contract | никаких implicit CPU fallback или backend-specific semantics |

Если backend не поддерживает операцию, Workbench/adapter обязан fail closed. Неподдерживаемый WebGPU/LinksDB путь не должен молча исполняться на CPU.

### Текущая архитектура Workbench

```text
Workbench UI
  -> ScenarioWorkerClient
  -> browser Worker
  -> real A-Circuit WASM
  -> CpuRuntimeSession
  -> OptimizedStructuralEngine

main thread:
  read-only catalog/presentation only
```

Worker владеет живой Session. Отмена и wall-clock watchdog наблюдаются как operational non-success; они не определяют semantic quiescence.

### Версии не смешиваются

```text
accepted MTS foundation:
  mts-contract/v0.14

execution profile:
  minimal-portable-amemory-execution
  profileVersion = 0.1.0
  foundationMtsVersion = v0.13

A-memory implementation:
  VERSION

contract/conformance candidate:
  amemory-contract/v0.2
  amemory-conformance/v0.2
```

Evidence/Scenario/proof schema versions — отдельные namespaces и не равны ни версии MTS, ни версии репозитория. Их текущие источники перечислены в `contracts/amemory-artifacts.json`.

### Что означает старый conformance evidence

`contracts/amemory-conformance-v0.2.json` сохраняет проверяемую candidate evidence-линейку, включая строку:

```text
FULL_REACTION_PROFILE_CONFORMANCE = TRUE
acceptanceState = READY
```

Это статус **candidate contract/conformance corpus**, а не заявление, что текущий Workbench уже имеет production/persistent WebGPU или LinksDB backend. Текущие capability-границы задаются таблицей выше и открытыми #266/#279/#280.

Детальная история R1–R7, старых prototype-срезов и промежуточных архитектур не дублируется в README: она остаётся в Git/PR/issue history и в machine-readable conformance evidence.

### Представления после MTS v0.14

```text
recursive Link wire != Anum / ExactSequence
```

Произвольная рекурсивная структура Link не должна называться Anum. `resultSequenceAnum` допустим только для значения, прошедшего sequence/ExactSequence representation. `START_K` / `END_K` — контекстно-относительные роли, а raw carrier start/end не являются semantic orientation authority.

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
