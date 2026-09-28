async function loadBuildInfo() {
  const versionNode = document.querySelector("#amemory-version");
  const shaNode = document.querySelector("#browser-lab-sha");
  const badge = document.querySelector("#amemory-build");

  try {
    const response = await fetch("./build-info.json", { cache: "no-store" });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const info = await response.json();

    if (info.schemaVersion !== 2 ||
        typeof info.version !== "string" ||
        !info.version.trim() ||
        typeof info.mainSha !== "string" ||
        !/^[0-9a-f]{40}$/.test(info.mainSha) ||
        !Number.isInteger(info.acceptanceRunId)) {
      throw new Error("unsupported or corrupt build-info schema");
    }

    const version = info.version.trim();
    const mainSha = info.mainSha;
    versionNode.textContent = `v${version}`;
    shaNode.textContent = mainSha.slice(0, 12);
    shaNode.title = mainSha;
    badge.title =
      `accepted web-lab run ${info.acceptanceRunId} · built ${info.builtAt || "unknown"} from ${mainSha}`;
  } catch (error) {
    versionNode.textContent = "version unavailable";
    shaNode.textContent = "unknown SHA";
    badge.title = `build identity unavailable: ${error.message}`;
  }
}

loadBuildInfo();
