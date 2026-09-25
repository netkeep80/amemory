export function assertExactU32(expected, actual, label = "differential") {
  const left = Array.from(expected, Number);
  const right = Array.from(actual, Number);

  if (left.length !== right.length) {
    throw new Error(`${label}: length mismatch ${left.length} != ${right.length}`);
  }

  for (let i = 0; i < left.length; i += 1) {
    if (left[i] !== right[i]) {
      throw new Error(
        `${label}: mismatch at index ${i}: expected ${left[i]}, got ${right[i]}`,
      );
    }
  }
  return true;
}

export function perturbFirst(values) {
  const copy = Uint32Array.from(values);
  if (copy.length === 0) {
    throw new Error("cannot perturb an empty result");
  }
  copy[0] ^= 1;
  return copy;
}
