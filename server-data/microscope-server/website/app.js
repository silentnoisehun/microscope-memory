const depthModel = [
  {
    id: "D0",
    name: "Identity",
    hint: "Core purpose and personality map of the memory space.",
    mag: "10X",
    cardA: "Identity Beacon",
    cardAText: "Core purpose and personality map of the memory space.",
    cardB: "Hall Map",
    cardBText: "Top-level summary map to orient the full palace quickly.",
  },
  {
    id: "D1",
    name: "Layer Summaries",
    hint: "Top-level summary map to orient the full palace quickly.",
    mag: "100X",
    cardA: "Hall Map",
    cardAText: "Top-level summary map to orient the full palace quickly.",
    cardB: "Cluster Rooms",
    cardBText: "Large semantic rooms where related themes gather.",
  },
  {
    id: "D2",
    name: "Topic Clusters",
    hint: "Large semantic rooms where related themes gather.",
    mag: "1,000X",
    cardA: "Cluster Rooms",
    cardAText: "Large semantic rooms where related themes gather.",
    cardB: "Memory Cells",
    cardBText: "Concrete chunks of user and agent memories.",
  },
  {
    id: "D3",
    name: "Memories",
    hint: "Concrete chunks of user and agent memories.",
    mag: "10,000X",
    cardA: "Memory Cells",
    cardAText: "Concrete chunks of user and agent memories.",
    cardB: "Sentence Strands",
    cardBText: "Fine detail where meaning can be reconstructed with context.",
  },
  {
    id: "D4",
    name: "Sentences",
    hint: "Fine detail where meaning can be reconstructed with context.",
    mag: "100,000X",
    cardA: "Sentence Strands",
    cardAText: "Fine detail where meaning can be reconstructed with context.",
    cardB: "Token Grid",
    cardBText: "Token-level fragments used for precise associative jumps.",
  },
  {
    id: "D5",
    name: "Tokens",
    hint: "Token-level fragments used for precise associative jumps.",
    mag: "250,000X",
    cardA: "Token Grid",
    cardAText: "Token-level fragments used for precise associative jumps.",
    cardB: "Syllable Pulse",
    cardBText: "Microscopic language grains for robust low-level linking.",
  },
  {
    id: "D6",
    name: "Syllables",
    hint: "Microscopic language grains for robust low-level linking.",
    mag: "500,000X",
    cardA: "Syllable Pulse",
    cardAText: "Microscopic language grains for robust low-level linking.",
    cardB: "Character Mesh",
    cardBText: "Character-space alignment for exact reconstruction paths.",
  },
  {
    id: "D7",
    name: "Characters",
    hint: "Character-space alignment for exact reconstruction paths.",
    mag: "750,000X",
    cardA: "Character Mesh",
    cardAText: "Character-space alignment for exact reconstruction paths.",
    cardB: "Byte Lattice",
    cardBText: "Atomic binary substrate where every memory can be anchored.",
  },
  {
    id: "D8",
    name: "Raw Bytes",
    hint: "Atomic binary floor",
    mag: "1,000,000X",
    cardA: "Byte Lattice",
    cardAText: "Atomic binary substrate where every memory can be anchored.",
    cardB: "Identity Beacon",
    cardBText: "The loop closes: raw bytes rebuild identity.",
  },
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

function bindCopyButtons() {
  const buttons = document.querySelectorAll(".copy-btn");
  if (!buttons.length) return;
  buttons.forEach((btn) => {
    btn.addEventListener("click", async () => {
      const targetId = btn.getAttribute("data-copy-target");
      if (!targetId) return;
      const target = document.getElementById(targetId);
      if (!target) return;
      const text = target.textContent || "";
      try {
        await navigator.clipboard.writeText(text);
        const old = btn.textContent;
        btn.textContent = "Copied";
        setTimeout(() => {
          btn.textContent = old;
        }, 1200);
      } catch (_error) {
        btn.textContent = "Copy failed";
      }
    });
  });
}

function bindMicroscopeDive() {
  const steps = Array.from(document.querySelectorAll(".layer-step"));
  if (!steps.length) return;

  const lensDepth = document.getElementById("lensDepth");
  const lensName = document.getElementById("lensName");
  const lensHint = document.getElementById("lensHint");
  const lensMag = document.getElementById("lensMag");
  const lensHeroTitle = document.getElementById("lensHeroTitle");
  const lensHeroText = document.getElementById("lensHeroText");
  const lensHeroCta = document.getElementById("lensHeroCta");
  const hudX = document.getElementById("hudX");
  const hudY = document.getElementById("hudY");
  const scopeFrame = document.querySelector(".scope-frame");
  const slider = document.getElementById("depthRange");
  const depthId = document.getElementById("depthId");
  const depthName = document.getElementById("depthName");
  const depthHint = document.getElementById("depthHint");

  const setActive = (depthIndex) => {
    const idx = Number(depthIndex);
    const data = depthModel[idx] || depthModel[0];
    steps.forEach((step, i) => {
      step.classList.toggle("is-active", i === idx);
      step.classList.toggle("image-live", i === idx);
    });
    if (lensDepth) lensDepth.textContent = data.id;
    if (lensName) lensName.textContent = data.name;
    if (lensHint) lensHint.textContent = data.hint;
    if (lensMag) lensMag.textContent = "MAGNIFICATION: " + data.mag;
    if (lensHeroTitle) {
      lensHeroTitle.textContent = idx === 8 ? "THE SOURCE CODE" : data.name.toUpperCase();
    }
    if (lensHeroText) {
      lensHeroText.textContent = idx === 8
        ? "Explore the architecture behind the lens. Open source and ready for your contribution."
        : data.hint;
    }
    if (lensHeroCta) {
      lensHeroCta.textContent = idx === 8 ? "VIEW ON GITHUB" : "ENTER " + data.id;
      lensHeroCta.href = idx === 8
        ? "https://github.com/silentnoisehun/microscope-memory"
        : "#microscope";
      lensHeroCta.target = idx === 8 ? "_blank" : "_self";
      lensHeroCta.rel = idx === 8 ? "noreferrer" : "";
    }

    if (slider) slider.value = String(idx);
    if (depthId) depthId.textContent = data.id;
    if (depthName) depthName.textContent = data.name;
    if (depthHint) depthHint.textContent = data.hint;

    const t = Math.max(0, Math.min(1, idx / 8));
    const bgZoom = (1 + t * 0.22).toFixed(3);
    const lensZoom = (1 + t * 0.1).toFixed(3);
    const tilt = (-t * 3.5).toFixed(2) + "deg";
    document.documentElement.style.setProperty("--bg-zoom", bgZoom);
    document.documentElement.style.setProperty("--lens-zoom", lensZoom);
    document.documentElement.style.setProperty("--track-tilt", tilt);
  };

  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (!entry.isIntersecting) return;
        const depth = entry.target.getAttribute("data-depth");
        if (depth == null) return;
        setActive(depth);
      });
    },
    {
      threshold: 0.6,
      rootMargin: "-20% 0px -20% 0px",
    }
  );

  steps.forEach((step) => observer.observe(step));
  setActive(8);

  if (scopeFrame && hudX && hudY) {
    scopeFrame.addEventListener("mousemove", (event) => {
      const rect = scopeFrame.getBoundingClientRect();
      const x = ((event.clientX - rect.left) / rect.width) * 1000;
      const y = ((event.clientY - rect.top) / rect.height) * 1000;
      hudX.textContent = x.toFixed(1);
      hudY.textContent = y.toFixed(1);
    });
  }
}

function bindAuth() {
  const guestBtn = document.getElementById("guestAuthBtn");
  const googleBtn = document.getElementById("googleAuthBtn");
  const appleBtn = document.getElementById("appleAuthBtn");
  const signOutBtn = document.getElementById("signOutBtn");
  const connectBtn = document.getElementById("connectMemoryBtn");
  const backendSelect = document.getElementById("memoryBackendSelect");
  const scopeSelect = document.getElementById("memoryScopeSelect");
  const status = document.getElementById("authStatus");
  const sessionStatus = document.getElementById("sessionStatus");
  if (!googleBtn || !appleBtn || !signOutBtn || !status || !sessionStatus) return;

  const demoKey = "microscope_demo_user_v1";
  const backendKey = "microscope_memory_backend_v1";
  const scopeKey = "microscope_memory_scope_v1";
  const baseUrl = (() => {
    const configured = (window.MICROSCOPE_API && window.MICROSCOPE_API.baseUrl) || "";
    return configured.trim() || window.location.origin;
  })();

  const getBackend = () => {
    if (backendSelect && backendSelect.value) return backendSelect.value;
    return localStorage.getItem(backendKey) || "cloud";
  };
  const getScope = () => {
    if (scopeSelect && scopeSelect.value) return scopeSelect.value;
    return localStorage.getItem(scopeKey) || "both";
  };
  const setBackend = (backend) => {
    localStorage.setItem(backendKey, backend);
    if (backendSelect) backendSelect.value = backend;
  };
  const setScope = (scope) => {
    localStorage.setItem(scopeKey, scope);
    if (scopeSelect) scopeSelect.value = scope;
  };
  const statusFor = (user, backend) => {
    const scope = user.scope || getScope();
    return "Signed in as " + user.name + " | own space: " + backend + " / " + scope + ".";
  };
  const updateSessionStatus = (text) => {
    sessionStatus.textContent = text;
  };
  const getCurrentUser = () => {
    if (window.MICROSCOPE_SESSION && window.MICROSCOPE_SESSION.userId) {
      const sid = window.MICROSCOPE_SESSION.userId;
      return {
        id: sid,
        name: sid,
      };
    }
    const existing = readDemoUser();
    if (existing) {
      return {
        id: existing.id,
        name: existing.name,
      };
    }
    return null;
  };
  const connectNamespace = async () => {
    const user = getCurrentUser();
    if (!user) {
      updateSessionStatus("Sign in first to connect namespace.");
      return;
    }
    const backend = getBackend();
    const scope = getScope();
    updateSessionStatus("Connecting " + backend + " / " + scope + " namespace...");
    try {
      const url = new URL("/v1/session", baseUrl);
      url.searchParams.set("user_id", user.id);
      url.searchParams.set("memory_backend", backend);
      url.searchParams.set("memory_scope", scope);
      const response = await fetch(url.toString(), { method: "GET" });
      if (!response.ok) {
        throw new Error("HTTP " + response.status);
      }
      const session = await response.json();
      window.MICROSCOPE_SESSION = {
        userId: session.user_id,
        backend: session.memory_backend,
        scope: session.memory_scope,
        namespaceDir: session.namespace_dir,
        personalNamespaceDir: session.personal_namespace_dir,
        sharedNamespaceDir: session.shared_namespace_dir,
      };
      updateSessionStatus(
        "Connected: personal=" +
          session.personal_namespace_dir +
          " | shared=" +
          session.shared_namespace_dir
      );
    } catch (error) {
      updateSessionStatus(
        "Cloud connection failed (" +
          (error.message || error) +
          "). Demo mode still works locally."
      );
    }
  };
  const setDemoUser = (provider) => {
    const id = Math.random().toString(36).slice(2, 10);
    const backend = getBackend();
    const scope = getScope();
    const user = {
      id,
      provider,
      name: "Guest-" + id,
      backend,
      scope,
      createdAt: Date.now(),
    };
    localStorage.setItem(demoKey, JSON.stringify(user));
    window.MICROSCOPE_SESSION = {
      userId: user.id,
      backend,
      scope,
    };
    status.textContent = statusFor(user, backend) + " (" + provider + " demo)";
    connectNamespace();
  };
  const readDemoUser = () => {
    try {
      const raw = localStorage.getItem(demoKey);
      return raw ? JSON.parse(raw) : null;
    } catch (_error) {
      return null;
    }
  };
  const clearDemoUser = () => {
    localStorage.removeItem(demoKey);
    status.textContent = "Signed out.";
    window.MICROSCOPE_SESSION = null;
    updateSessionStatus("Cloud session not connected yet.");
  };

  setBackend(localStorage.getItem(backendKey) || "cloud");
  setScope(localStorage.getItem(scopeKey) || "both");
  if (backendSelect) {
    backendSelect.addEventListener("change", () => {
      const backend = backendSelect.value === "cloud" ? "cloud" : "local";
      setBackend(backend);
      const existing = readDemoUser();
      if (existing) {
        existing.backend = backend;
        localStorage.setItem(demoKey, JSON.stringify(existing));
        status.textContent = statusFor(existing, backend);
        connectNamespace();
      }
    });
  }
  if (scopeSelect) {
    scopeSelect.addEventListener("change", () => {
      const scope = scopeSelect.value === "personal" || scopeSelect.value === "shared"
        ? scopeSelect.value
        : "both";
      setScope(scope);
      const existing = readDemoUser();
      if (existing) {
        existing.scope = scope;
        localStorage.setItem(demoKey, JSON.stringify(existing));
        status.textContent = statusFor(existing, getBackend());
        connectNamespace();
      }
    });
  }
  if (connectBtn) {
    connectBtn.addEventListener("click", () => {
      connectNamespace();
    });
  }

  const authConfig = window.MICROSCOPE_AUTH || {};
  const firebaseConfig = authConfig.firebaseConfig || {};
  const required = ["apiKey", "authDomain", "projectId", "appId"];
  const configured = required.every((key) => typeof firebaseConfig[key] === "string" && firebaseConfig[key].trim() !== "");

  if (!authConfig.enabled || !configured || !window.firebase) {
    const existing = readDemoUser();
    if (existing) {
      const backend = existing.backend || getBackend();
      setBackend(backend);
      window.MICROSCOPE_SESSION = {
        userId: existing.id,
        backend,
        scope: existing.scope || getScope(),
      };
      status.textContent = statusFor(existing, backend) + " (" + existing.provider + " demo)";
      connectNamespace();
    } else {
      status.textContent = "Instant mode active: click any button and continue.";
      updateSessionStatus("Cloud session not connected yet.");
    }

    if (guestBtn) {
      guestBtn.addEventListener("click", () => setDemoUser("instant"));
    }
    googleBtn.addEventListener("click", () => setDemoUser("google"));
    appleBtn.addEventListener("click", () => setDemoUser("apple"));
    signOutBtn.addEventListener("click", clearDemoUser);
    return;
  }

  try {
    if (!window.firebase.apps.length) {
      window.firebase.initializeApp(firebaseConfig);
    }
  } catch (error) {
    status.textContent = "Firebase init failed: " + (error.message || error);
    googleBtn.disabled = true;
    appleBtn.disabled = true;
    signOutBtn.disabled = true;
    return;
  }

  const auth = window.firebase.auth();
  const googleProvider = new window.firebase.auth.GoogleAuthProvider();
  const appleProvider = new window.firebase.auth.OAuthProvider("apple.com");

  auth.onAuthStateChanged((user) => {
    if (!user) {
      status.textContent = "Signed out.";
      window.MICROSCOPE_SESSION = null;
      updateSessionStatus("Cloud session not connected yet.");
      return;
    }
    const backend = getBackend();
    const scope = getScope();
    const name = user.displayName || user.email || user.uid;
    window.MICROSCOPE_SESSION = {
      userId: user.uid || name,
      backend,
      scope,
    };
    status.textContent = "Signed in as " + name + " | own space: " + backend + " / " + scope + ".";
    connectNamespace();
  });

  googleBtn.addEventListener("click", async () => {
    status.textContent = "Opening Google sign-in...";
    try {
      await auth.signInWithPopup(googleProvider);
      status.textContent = "Google sign-in success.";
    } catch (error) {
      status.textContent = "Google sign-in failed: " + (error.message || error);
    }
  });

  appleBtn.addEventListener("click", async () => {
    status.textContent = "Opening Apple sign-in...";
    try {
      await auth.signInWithPopup(appleProvider);
      status.textContent = "Apple sign-in success.";
    } catch (error) {
      status.textContent = "Apple sign-in failed: " + (error.message || error);
    }
  });

  signOutBtn.addEventListener("click", async () => {
    try {
      await auth.signOut();
      status.textContent = "Signed out.";
    } catch (error) {
      status.textContent = "Sign-out failed: " + (error.message || error);
    }
  });

  if (guestBtn) {
    guestBtn.addEventListener("click", () => {
      setDemoUser("instant");
    });
  }
}

bindDepthTool();
bindReveal();
bindSmoothNav();
bindCopyButtons();
bindAuth();
bindMicroscopeDive();
