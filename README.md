# amemory

Библиотека реализаций и симуляций **A-memory**.

## Назначение

`amemory` не задаёт семантику MTS. Нормативная теория, законы и доказательства находятся в:

- https://github.com/netkeep80/anum_docs

Этот репозиторий исследует и реализует различные физические и вычислительные способы исполнения A-memory при одном обязательном условии:

> каждая реализация явно заявляет поддерживаемую версию MTS и должна воспроизводить её наблюдаемую семантику.

## Главный принцип

```text
one MTS version
      |
      +--> reference CPU simulator
      +--> optimized CPU implementation
      +--> Links/Doublets backend
      +--> GPU associative simulator
      +--> future specialized architecture
```

Внутреннее устройство реализаций **не обязано совпадать**.

Например:

- Links Platform / Doublets может хранить Links как канонические дуплеты;
- purpose-built backend может использовать специализированные индексы и layouts;
- GPU backend может симулировать новую ассоциативную архитектуру и вообще не использовать дуплет как физическую вычислительную ячейку.

Обязательным является не внутреннее представление, а соответствие заявленной версии MTS.

## Conformance

Каждая реализация должна объявлять как минимум:

```text
supported MTS version
exact normative/conformance revision
implementation status
```

Статус `MTS-conformant` допустим только после прохождения общего conformance/differential corpus для этой версии.

Экспериментальная реализация может поддерживать только часть поведения, но должна явно обозначать свой scope и не выдаваться за полную реализацию версии MTS.

## Первые направления

1. **Reference CPU simulator** — простой и проверяемый oracle.
2. **Optimized CPU backend** — специализированная реализация A-memory.
3. **Links/Doublets backend** — исследование готовой инфраструктуры дуплетов:
   - https://github.com/linksplatform/doublets-web — WebAssembly для JS/TS;
   - https://github.com/linksplatform/doublets-rs — Rust/native implementation candidate.
4. **Associative accelerator simulator** — GPU/iGPU-симуляция новой ассоциативной архитектуры A-memory.
5. В дальнейшем — другие accelerator и специализированные hardware backends.

## Граница репозиториев

```text
anum_docs
  theory + proofs + normative conformance
       |
       v
amemory
  implementations + simulations + benchmarks
       |
       v
aprover
  consumer of a conformant A-memory implementation
```

`amemory` может выявлять проблемы или новые гипотезы, но изменение семантики MTS должно возвращаться в `anum_docs`.


## Upstream-исследования и границы реализации

У `amemory` сейчас есть две независимые теоретические зависимости в `anum_docs`:

1. **netkeep80/anum_docs#1558 — полная динамика исполняемой A-сети.**

   Отвечает на вопрос, что именно значит исполнять долгоживущую A-сеть: semantic current state, переходы состояния, ZERO/ONE/MANY, параллельные реакции, live Theory mutation, quiescence/recurrence и граница между логическим состоянием и физической историей Links.

   Эта задача относится ко **всем полным реализациям A-memory**. До её классификации реализации могут поддерживать accepted/scoped execution MTS v0.13, но не должны молча объявлять более сильную семантику нормативной.

2. **netkeep80/anum_docs#1332 — преобразование MTS Aset <-> Doublets Aset.**

   Отвечает на вопрос корректного отображения между semantic Link identity MTS и identity элементов сети дуплетов, включая root/acorn, membership, shared topology, cycles, duplicate-pair identity и contextual isolation.

   Эта задача относится прежде всего к **Links/Doublets backend**. Она не блокирует reference CPU, purpose-built CPU или associative accelerator backend.

Итого:

```text
#1558  execution semantics
        └─ влияет на полноту A-memory как исполняемой системы

#1332  representation/substrate mapping
        └─ влияет на полноту Links/Doublets backend
```

Если реализация обнаруживает новый семантический пробел, он возвращается в `anum_docs`, а не закрепляется локально как новая семантика `amemory`.
