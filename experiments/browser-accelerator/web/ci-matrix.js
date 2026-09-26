(() => {
  "use strict";

  const root = document.querySelector("main");
  if (!root) return;

  const style = document.createElement("style");
  style.textContent = `
    .ci-matrix { margin-top: 36px; }
    .ci-matrix .ci-summary {
      display: grid;
      grid-template-columns: repeat(4, minmax(0, 1fr));
      gap: 12px;
      margin: 14px 0;
    }
    .ci-matrix .ci-card {
      background: var(--surface);
      border: 1px solid var(--line);
      border-radius: 14px;
      padding: 14px 16px;
      box-shadow: var(--shadow);
    }
    .ci-matrix .ci-kicker {
      display: block;
      color: var(--muted);
      font-size: .74rem;
      text-transform: uppercase;
      letter-spacing: .08em;
    }
    .ci-matrix .ci-value {
      display: block;
      margin-top: 7px;
      font-weight: 760;
      overflow-wrap: anywhere;
    }
    .ci-matrix .ci-table-wrap {
      overflow-x: auto;
      background: var(--surface);
      border: 1px solid var(--line);
      border-radius: 14px;
      box-shadow: var(--shadow);
    }
    .ci-matrix table {
      width: 100%;
      border-collapse: collapse;
      min-width: 720px;
    }
    .ci-matrix th,
    .ci-matrix td {
      text-align: left;
      padding: 10px 13px;
      border-bottom: 1px solid var(--line);
      vertical-align: top;
    }
    .ci-matrix tr:last-child td { border-bottom: 0; }
    .ci-matrix th {
      color: var(--muted);
      font-size: .78rem;
      text-transform: uppercase;
      letter-spacing: .06em;
    }
    .ci-matrix a { color: var(--accent); }
    .ci-matrix .ci-good { color: var(--good); font-weight: 760; }
    .ci-matrix .ci-bad { color: var(--bad); font-weight: 760; }
    .ci-matrix .ci-pending { color: var(--muted); font-weight: 760; }
    .ci-matrix .ci-meta { color: var(--muted); font-size: .86rem; }
    @media (max-width: 820px) {
      .ci-matrix .ci-summary { grid-template-columns: 1fr 1fr; }
    }
    @media (max-width: 520px) {
      .ci-matrix .ci-summary { grid-template-columns: 1fr; }
    }
  `;
  document.head.append(style);

  const section = document.createElement("section");
  section.className = "ci-matrix";
  section.setAttribute("aria-label", "Repository CI test matrix");

  const title = document.createElement("h2");
  title.textContent = "Repository test matrix";
  section.append(title);

  const intro = document.createElement("p");
  intro.textContent =
    "Automatically generated from GitHub check-runs for the exact revision merged into main. " +
    "This is CI evidence; the CPU/WASM ↔ WebGPU witnesses above execute locally in this browser.";
  section.append(intro);

  const status = document.createElement("div");
  status.className = "notice";
  status.textContent = "Loading exact-head CI evidence…";
  section.append(status);

  root.append(section);

  const shortSha = (value) => typeof value === "string" && value.length >= 8 ? value.slice(0, 12) : "unknown";

  const classify = (check) => {
    if (check.status !== "completed") return { label: check.status || "pending", className: "ci-pending" };
    const conclusion = check.conclusion || "unknown";
    if (["success", "neutral", "skipped"].includes(conclusion)) {
      return { label: conclusion, className: "ci-good" };
    }
    return { label: conclusion, className: "ci-bad" };
  };

  const card = (label, value) => {
    const node = document.createElement("div");
    node.className = "ci-card";
    const kicker = document.createElement("span");
    kicker.className = "ci-kicker";
    kicker.textContent = label;
    const content = document.createElement("span");
    content.className = "ci-value";
    content.textContent = value;
    node.append(kicker, content);
    return node;
  };

  const safeGithubUrl = (value) =>
    typeof value === "string" && /^https:\/\/github\.com\//.test(value) ? value : null;

  fetch("./ci-results.json", { cache: "no-store" })
    .then((response) => {
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      return response.json();
    })
    .then((data) => {
      const checks = Array.isArray(data.checks) ? data.checks : [];
      const completed = checks.filter((check) => check.status === "completed");
      const passing = completed.filter((check) =>
        ["success", "neutral", "skipped"].includes(check.conclusion)
      ).length;
      const failing = completed.length - passing;
      const pending = checks.length - completed.length;

      status.remove();

      const summary = document.createElement("div");
      summary.className = "ci-summary";
      summary.append(
        card("Checks", String(checks.length)),
        card("Passing", String(passing)),
        card("Failing", String(failing)),
        card("Pending", String(pending)),
      );
      section.append(summary);

      const source = document.createElement("p");
      source.className = "ci-meta";
      const sourceKind = data.source?.kind === "merged_pr_head"
        ? `merged PR #${data.source.prNumber ?? "?"} head`
        : "main revision";
      source.textContent =
        `Source: ${sourceKind} ${shortSha(data.sourceSha)} · main ${shortSha(data.mainSha)} · generated ${data.generatedAt || "unknown"}`;
      section.append(source);

      if (checks.length === 0) {
        const empty = document.createElement("div");
        empty.className = "notice fail";
        empty.textContent = "No GitHub check-runs were found for the exact source revision.";
        section.append(empty);
        return;
      }

      const wrap = document.createElement("div");
      wrap.className = "ci-table-wrap";
      const table = document.createElement("table");
      const thead = document.createElement("thead");
      const header = document.createElement("tr");
      for (const label of ["Check", "Result", "App", "Details"]) {
        const th = document.createElement("th");
        th.textContent = label;
        header.append(th);
      }
      thead.append(header);
      table.append(thead);

      const tbody = document.createElement("tbody");
      for (const check of checks) {
        const row = document.createElement("tr");

        const name = document.createElement("td");
        name.textContent = check.name || "(unnamed check)";

        const result = document.createElement("td");
        const classification = classify(check);
        result.textContent = classification.label;
        result.className = classification.className;

        const app = document.createElement("td");
        app.textContent = check.app || "unknown";

        const details = document.createElement("td");
        const href = safeGithubUrl(check.detailsUrl);
        if (href) {
          const link = document.createElement("a");
          link.href = href;
          link.target = "_blank";
          link.rel = "noreferrer";
          link.textContent = "open";
          details.append(link);
        } else {
          details.textContent = "—";
        }

        row.append(name, result, app, details);
        tbody.append(row);
      }
      table.append(tbody);
      wrap.append(table);
      section.append(wrap);
    })
    .catch((error) => {
      status.textContent = `CI evidence unavailable: ${error.message}`;
      status.classList.add("fail");
    });
})();
