export function normalizePairSet(pairs) {
  const keys = pairs.map(([start, end]) => `${Number(start)}:${Number(end)}`);
  const unique = [...new Set(keys)].sort();
  return unique;
}

export function newPairsFromFlags(candidates, flags) {
  if (candidates.length !== flags.length) {
    throw new Error(
      `candidate/flag length mismatch: ${candidates.length} != ${flags.length}`,
    );
  }
  const pairs = [];
  for (let i = 0; i < candidates.length; i += 1) {
    if ((Number(flags[i]) & 4) !== 0) {
      pairs.push(candidates[i]);
    }
  }
  return pairs;
}

export function assertCanonicalNewPairs(expectedPairs, actualPairs, label = "canonical pairs") {
  const expected = normalizePairSet(expectedPairs);
  const actual = normalizePairSet(actualPairs);

  if (actual.length !== actualPairs.length) {
    throw new Error(`${label}: duplicate pair present in canonical output`);
  }
  if (expected.length !== actual.length) {
    throw new Error(`${label}: pair-count mismatch ${expected.length} != ${actual.length}`);
  }
  for (let i = 0; i < expected.length; i += 1) {
    if (expected[i] !== actual[i]) {
      throw new Error(
        `${label}: mismatch at normalized index ${i}: expected ${expected[i]}, got ${actual[i]}`,
      );
    }
  }
  return true;
}
