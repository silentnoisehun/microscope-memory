const depthModel = [
  { id: "D0", name: "Identity", hint: "Root identity and intent layer" },
  { id: "D1", name: "Layer Summaries", hint: "High-level map of memory zones" },
  { id: "D2", name: "Topic Clusters", hint: "Rooms and neighborhoods of knowledge" },
  { id: "D3", name: "Memories", hint: "Room-level concepts" },
  { id: "D4", name: "Sentences", hint: "Detailed content granularity" },
  { id: "D5", name: "Tokens", hint: "Fine-grained retrieval atoms" },
  { id: "D6", name: "Syllables", hint: "Microscopic language fragments" },
  { id: "D7", name: "Characters", hint: "Character-level context" },
  { id: "D8", name: "Raw Bytes", hint: "Atomic binary floor" },
];

function bindDepthTool() {
  const slider = document.getElementById("depthRange");
  const depthId = document.getElementById("depthId");
  const depthName = document.getElementById("depthName");
  const depthHint = document.getElementById("depthHint");
  if (!slider || !depthId || !depthName || !depthHint) return;

  const render = () => {
    const value = Number(slider.value);
    const data = depthModel[value] || depthModel[3];
    depthId.textContent = data.id;
    depthName.textContent = data.name;
    depthHint.textContent = data.hint;
  };

  slider.addEventListener("input", render);
  render();
}

function bindReveal() {
  const nodes = document.querySelectorAll(".reveal");
  if (!nodes.length) return;

  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          entry.target.classList.add("show");
          observer.unobserve(entry.target);
        }
      }
    },
    { threshold: 0.18 }
  );

  nodes.forEach((n) => observer.observe(n));
}

function bindSmoothNav() {
  document.querySelectorAll('a[href^="#"]').forEach((anchor) => {
    anchor.addEventListener("click", (event) => {
      const href = anchor.getAttribute("href");
      if (!href || href === "#") return;
      const target = document.querySelector(href);
      if (!target) return;
      event.preventDefault();
      target.scrollIntoView({ behavior: "smooth", block: "start" });
    });
  });
}

bindDepthTool();
bindReveal();
bindSmoothNav();
