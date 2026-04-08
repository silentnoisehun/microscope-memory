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

    if (slider) slider.value = String(idx);
    if (depthId) depthId.textContent = data.id;
    if (depthName) depthName.textContent = data.name;
    if (depthHint) depthHint.textContent = data.hint;
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
  setActive(0);
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
