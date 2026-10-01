import fs from "node:fs";

const page = fs.readFileSync(
  "experiments/browser-accelerator/web/index.html",
  "utf8",
);
const readme = fs.readFileSync("README.md", "utf8");

for (const required of [
  "∞ = ROOT / 8",
  "♂S = START_K / 9S",
  "S♀ = END_K / 6S",
  "A ⟼ B = PAIR / 1AB",
  "recursive Link wire ≠ Anum / ExactSequence",
]) {
  if (!page.includes(required)) {
    throw new Error("dashboard lost v0.14 notation: " + required);
  }
}

for (const required of [
  "START_K",
  "END_K",
  "∞",
  "♂",
  "♀",
  "⟼",
  "recursive Link wire",
  "Anum / ExactSequence",
]) {
  if (!readme.includes(required)) {
    throw new Error("README lost v0.14 terminology: " + required);
  }
}

for (const forbidden of [
  "Anum boundary",
  "Multi-Anum",
  "Scope Anums",
  "Anum import CPU:",
  "Anum export CPU:",
  "Two-memory Anum differential:",
]) {
  if (page.includes(forbidden)) {
    throw new Error(
      "legacy representation term leaked to public dashboard: " + forbidden,
    );
  }
}

console.log("MTS_V0_14_PUBLIC_TERMINOLOGY=PASS");
