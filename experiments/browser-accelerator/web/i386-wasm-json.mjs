function fail(label, message) {
  throw new Error(`${label} ABI: ${message}`);
}

export function readUtf8Abi(wasm, names, label) {
  const available = wasm[names.available];
  if (typeof available !== "function" || available() !== 1) return null;
  const length = wasm[names.length];
  if (typeof length !== "function") fail(label, "length function missing");
  const len = length() >>> 0;
  if (!len) fail(label, "reported empty UTF-8 payload");

  let bytes;
  const pointer = wasm[names.pointer];
  if (wasm.memory && typeof pointer === "function") {
    const ptr = pointer() >>> 0;
    if (ptr > wasm.memory.buffer.byteLength ||
        len > wasm.memory.buffer.byteLength - ptr) {
      fail(label, "pointer outside WASM memory");
    }
    bytes = new Uint8Array(wasm.memory.buffer, ptr, len);
  } else {
    const byte = wasm[names.byte];
    if (typeof byte !== "function") fail(label, "byte function missing");
    bytes = new Uint8Array(len);
    for (let i = 0; i < len; i += 1) {
      const value = byte(i) >>> 0;
      if (value > 255) fail(label, `invalid byte at ${i}`);
      bytes[i] = value;
    }
  }

  return new TextDecoder().decode(bytes);
}

export function readJsonAbi(wasm, names, label) {
  const text = readUtf8Abi(wasm, names, label);
  if (text === null) return null;

  try {
    return JSON.parse(text);
  } catch (error) {
    fail(label, `JSON parse failed: ${error.message}`);
  }
}
