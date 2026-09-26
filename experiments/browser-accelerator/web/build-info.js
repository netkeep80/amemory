async function loadBuildInfo() {
  const versionNode = document.querySelector("#amemory-version");
  const shaNode = document.querySelector("#browser-lab-sha");
  const badge = document.querySelector("#amemory-build");

  try {
    const response = await fetch("./build-info.json", { cache: "no-store" });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const info = await response.json();

    if (info.schemaVersion !== 1 || typeof info.version !== "string" || typeof info.mainSha !== "string") {
      throw new Error("unsupported build-info schema");
    }

    versionNode.textContent = `v${info.version}`;
    shaNode.textContent = info.mainSha.slice(0, 12);
    shaNode.title = info.mainSha;
    badge.title = `built ${info.builtAt || "unknown"} from ${info.mainSha}`;
  } catch (error) {
    versionNode.textContent = "version unavailable";
    shaNode.textContent = "unknown SHA";
    badge.title = `build identity unavailable: ${error.message}`;
  }
}

loadBuildInfo();
