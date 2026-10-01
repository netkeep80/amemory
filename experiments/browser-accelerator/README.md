# Browser accelerator / Workbench

Эта директория больше не является отдельным «первым WebGPU spike». Она содержит browser/Pages поверхность единого A-memory Workbench и исследования физических accelerator-backend.

История раннего spike не копируется в отдельный архивный документ: она остаётся в Git.

## Текущий исполняющий путь

```text
Workbench
  -> ScenarioWorkerClient
  -> browser Worker
  -> amemory-a-circuit WASM
  -> persistent optimized-CPU Session
  -> structural execution / result / evidence
```

Main thread не владеет второй живой Session. Допустим отдельный read-only catalog WASM для реестра сценариев и presentation metadata.

Повторные запуски одной загруженной асети используют ту же Session:

```text
PREPARE -> LOAD
        -> CONFIGURE -> EXECUTE -> RESULT
        -> CONFIGURE -> EXECUTE -> RESULT
        -> ...
```

Cancellation, resource budgets и wall-clock watchdog возвращают явный non-success/recovery state. Таймер не является semantic oracle.

## Backend capabilities

| Backend | Browser/Workbench capability |
|---|---|
| optimized CPU | поддержан как текущий persistent runtime |
| reference Rust/WASM | независимый reference/differential backend, не основной Workbench runtime |
| WebGPU | bounded research; #266 должен доказать authentic packed-carrier MUX1 `discover -> publish`; persistent adapter #279 ещё не реализован |
| LinksDB | persistent adapter #280 ещё не реализован |

Неподдерживаемый backend обязан fail closed. Silent CPU fallback запрещён.

## WebGPU boundary

Наличие WGSL, dispatch/readback или старого CPU↔GPU differential само по себе не означает полный WebGPU A-memory backend.

Сильный capability claim требует как минимум:

- реальный packed carrier из того же PREPARE path;
- structural discovery без expected-result seeding;
- отдельную PUBLISH-границу;
- browser compile/dispatch/readback;
- сравнение с независимым CPU/Rust evidence только **после** GPU исполнения;
- для persistent claim — resident Session lifecycle из #279.

## Версии и authority

Не смешивать:

```text
MTS foundation        = accepted v0.14
execution profile     = minimal-portable-amemory-execution / 0.1.0
profile foundation    = v0.13
amemory implementation= root VERSION
evidence/scenario     = собственные schemaVersion
```

MTS semantic authority находится в `netkeep80/anum_docs`. Browser code, GPU layout, local handles, Worker protocol и визуализация не являются semantic authority.

## Проверка browser поверхности

Pinned Rust toolchain:

```text
Rust 1.98.1
target: wasm32-unknown-unknown
```

Быстрая acceptance-проверка browser модулей и renderer/build identity:

```bash
node experiments/a-circuit/tests/workbench-browser-acceptance.mjs
```

Real browser retained-Session E2E запускается существующим `A-Circuit Workbench real-WASM` workflow после сборки настоящего A-Circuit WASM.

## GitHub Pages

Текущая публичная поверхность:

```text
https://netkeep80.github.io/amemory/
```

Pages должен публиковать exact accepted SHA/VERSION/evidence и тот же Workbench, а не отдельную legacy GPU страницу.
