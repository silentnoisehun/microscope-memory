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

function bindAuth() {
  const guestBtn = document.getElementById("guestAuthBtn");
  const googleBtn = document.getElementById("googleAuthBtn");
  const appleBtn = document.getElementById("appleAuthBtn");
  const signOutBtn = document.getElementById("signOutBtn");
  const status = document.getElementById("authStatus");
  if (!googleBtn || !appleBtn || !signOutBtn || !status) return;

  const demoKey = "microscope_demo_user_v1";
  const setDemoUser = (provider) => {
    const id = Math.random().toString(36).slice(2, 10);
    const user = {
      id,
      provider,
      name: "Guest-" + id,
      createdAt: Date.now(),
    };
    localStorage.setItem(demoKey, JSON.stringify(user));
    status.textContent = "Signed in instantly as " + user.name + " (" + provider + " demo).";
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
  };

  const authConfig = window.MICROSCOPE_AUTH || {};
  const firebaseConfig = authConfig.firebaseConfig || {};
  const required = ["apiKey", "authDomain", "projectId", "appId"];
  const configured = required.every((key) => typeof firebaseConfig[key] === "string" && firebaseConfig[key].trim() !== "");

  if (!authConfig.enabled || !configured || !window.firebase) {
    const existing = readDemoUser();
    if (existing) {
      status.textContent = "Signed in instantly as " + existing.name + " (" + existing.provider + " demo).";
    } else {
      status.textContent = "Instant mode active: click any button and continue.";
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
      return;
    }
    const name = user.displayName || user.email || user.uid;
    status.textContent = "Signed in as: " + name;
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
