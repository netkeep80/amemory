export const STATE_SIDE = 16;
export const STATE_CELLS = STATE_SIDE * STATE_SIDE;

export function validatePhysicalPair([start, end]) {
  return Number.isInteger(start) &&
    Number.isInteger(end) &&
    start >= 0 &&
    end >= 0 &&
    start < STATE_SIDE &&
    end < STATE_SIDE;
}

export function matrixToPairs(cells) {
  if (cells.length !== STATE_CELLS) {
    throw new Error(`state matrix size mismatch: expected ${STATE_CELLS}, got ${cells.length}`);
  }
  const pairs = [];
  for (let start = 0; start < STATE_SIDE; start += 1) {
    for (let end = 0; end < STATE_SIDE; end += 1) {
      const index = start * STATE_SIDE + end;
      if (Number(cells[index]) !== 0) pairs.push([start, end]);
    }
  }
  return pairs;
}

export function normalizeStatePairs(pairs) {
  return [...new Set(pairs.map(([start,end]) => `${Number(start)}:${Number(end)}`))].sort();
}

export function assertStateExact(expectedPairs, actualPairs, label = "state") {
  const expected = normalizeStatePairs(expectedPairs);
  const actual = normalizeStatePairs(actualPairs);

  if (actual.length !== actualPairs.length) {
    throw new Error(`${label}: duplicate portable pair observation`);
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

export function assertStatePreserved(previousPairs, nextPairs, label = "state persistence") {
  const next = new Set(normalizeStatePairs(nextPairs));
  for (const key of normalizeStatePairs(previousPairs)) {
    if (!next.has(key)) {
      throw new Error(`${label}: prior pair disappeared: ${key}`);
    }
  }
  return true;
}

export function applyAuthorizedModel(previousPairs, commits) {
  const next = [...previousPairs.map(([s,e]) => [s,e])];
  const seen = new Set(normalizeStatePairs(next));
  for (const pair of commits) {
    if (!validatePhysicalPair(pair)) continue;
    const key = `${pair[0]}:${pair[1]}`;
    if (!seen.has(key)) {
      seen.add(key);
      next.push([pair[0], pair[1]]);
    }
  }
  return next;
}

export function formatStatePairs(pairs) {
  return "[" +
    normalizeStatePairs(pairs)
      .map((key) => "(" + key.replace(":", ",") + ")")
      .join(", ") +
    "]";
}
