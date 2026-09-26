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
      min-width: 860px;
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
    .ci-matrix .ci-suite-path {
      display: block;
      margin-top: 3px;
      color: var(--muted);
      font-size: .76rem;
      font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
    }
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
  section.setAttribute("aria-label", "Repository CI workflow matrix");

  const title = document.createElement("h2");
  title.textContent = "Repository test matrix";
  section.append(title);

  const intro = document.createElement("p");
  intro.textContent =
    "Automatically generated from every active GitHub Actions workflow. " +
    "Each suite shows its latest main run when available, otherwise its latest run on any branch. " +
    "The Pages deployment workflow itself is excluded.";
  section.append(intro);

  const status = document.createElement("div");
  status.className = "notice";
  status.textContent = "Loading repository workflow evidence…";
  section.append(status);

  root.append(section);

  const shortSha = (value) =>
    typeof value === "string" && value.length >= 8 ? value.slice(0, 12) : "unknown";

  const classify = (workflow) => {
    const run = workflow.run;
    if (!run) return { label: "never run", className: "ci-pending" };
    if (run.status !== "completed") {
      return { label: run.status || "pending", className: "ci-pending" };
    }
    const conclusion = run.conclusion || "unknown";
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
      if (data.schemaVersion !== 2) {
        throw new Error(`unsupported CI manifest schema ${data.schemaVersion ?? "unknown"}`);
      }

      const workflows = Array.isArray(data.workflows) ? data.workflows : [];
      const completed = workflows.filter((workflow) => workflow.run?.status === "completed");
      const passing = completed.filter((workflow) =>
        ["success", "neutral", "skipped"].includes(workflow.run?.conclusion)
      ).length;
      const failing = completed.length - passing;
      const pending = workflows.length - completed.length;

      status.remove();

      const summary = document.createElement("div");
      summary.className = "ci-summary";
      summary.append(
        card("Suites", String(workflows.length)),
        card("Passing", String(passing)),
        card("Failing", String(failing)),
        card("Pending / never", String(pending)),
      );
      section.append(summary);

      const source = document.createElement("p");
      source.className = "ci-meta";
      const sourceKind = data.source?.kind === "merged_pr_head"
        ? `deployment from merged PR #${data.source.prNumber ?? "?"} head`
        : "deployment from main revision";
      source.textContent =
        `${sourceKind} ${shortSha(data.sourceSha)} · main ${shortSha(data.mainSha)} · generated ${data.generatedAt || "unknown"}`;
      section.append(source);

      if (workflows.length === 0) {
        const empty = document.createElement("div");
        empty.className = "notice fail";
        empty.textContent = "No active GitHub Actions workflows were found.";
        section.append(empty);
        return;
      }

      const wrap = document.createElement("div");
      wrap.className = "ci-table-wrap";
      const table = document.createElement("table");
      const thead = document.createElement("thead");
      const header = document.createElement("tr");
      for (const label of ["Suite", "Result", "Latest source", "Details"]) {
        const th = document.createElement("th");
        th.textContent = label;
        header.append(th);
      }
      thead.append(header);
      table.append(thead);

      const tbody = document.createElement("tbody");
      for (const workflow of workflows) {
        const row = document.createElement("tr");

        const suite = document.createElement("td");
        const suiteName = document.createElement("span");
        suiteName.textContent = workflow.name || "(unnamed workflow)";
        suite.append(suiteName);
        if (workflow.path) {
          const suitePath = document.createElement("span");
          suitePath.className = "ci-suite-path";
          suitePath.textContent = workflow.path;
          suite.append(suitePath);
        }

        const result = document.createElement("td");
        const classification = classify(workflow);
        result.textContent = classification.label;
        result.className = classification.className;

        const latestSource = document.createElement("td");
        if (workflow.run) {
          const branch = workflow.run.headBranch || "(no branch)";
          const event = workflow.run.event || "unknown event";
          latestSource.textContent =
            `${event} · ${branch} · ${shortSha(workflow.run.headSha)}`;
        } else {
          latestSource.textContent = "—";
        }

        const details = document.createElement("td");
        const href = safeGithubUrl(workflow.run?.url);
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

        row.append(suite, result, latestSource, details);
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
