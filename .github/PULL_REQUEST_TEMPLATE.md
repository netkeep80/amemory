## Краткое описание

<!-- Что меняется и зачем? -->

## Намерение изменения

```repo-guard-yaml
change_type: feature
scope:
  - src/**
budgets: {}
anchors:
  affects: []
  implements: []
  verifies: []
must_touch: []
must_not_touch:
  - contracts/**
expected_effects:
  - Опишите наблюдаемый эффект
```

Для изменения contract/conformance или другой governance-поверхности используйте связанную задачу с явным разрешением.
