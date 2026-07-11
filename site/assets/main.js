/* TokenHub site — segment rendering, prism flip, dropdown, clock, reveals */
(function () {
  "use strict";

  var reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  /* ---------- build segmented bars from data-fill ---------- */
  var SEG_COUNT = 10;
  document.querySelectorAll("[data-segs]").forEach(function (el) {
    var fill = Math.max(0, Math.min(100, parseFloat(el.dataset.fill) || 0));
    var exact = (fill / 100) * SEG_COUNT;
    var full = Math.floor(exact);
    var part = exact - full;
    for (var i = 0; i < SEG_COUNT; i++) {
      var s = document.createElement("i");
      s.className = "seg";
      s.style.setProperty("--i", i);
      if (i < full) {
        s.classList.add("seg--fill");
      } else if (i === full && part > 0.12) {
        s.classList.add("seg--part");
        s.style.setProperty("--part", Math.round(part * 100) + "%");
      }
      el.appendChild(s);
    }
  });

  /* ---------- 14-day trend bars ---------- */
  var trend = document.querySelector("[data-trend]");
  if (trend) {
    var heights = [34, 52, 41, 68, 47, 22, 30, 74, 58, 82, 64, 40, 90, 71];
    heights.forEach(function (h, i) {
      var b = document.createElement("i");
      b.style.setProperty("--h", h);
      b.style.setProperty("--i", i);
      trend.appendChild(b);
    });
  }

  /* ---------- big bar fill targets ---------- */
  document.querySelectorAll("[data-bb]").forEach(function (el) {
    el.style.setProperty("--w", el.dataset.bb);
  });

  /* ---------- taskbar clock (visitor's real time) ---------- */
  var clockTime = document.querySelector("[data-clock-time]");
  var clockDate = document.querySelector("[data-clock-date]");
  function tick() {
    var now = new Date();
    if (clockTime) {
      clockTime.textContent = now.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
    }
    if (clockDate) {
      clockDate.textContent = now.toLocaleDateString();
    }
  }
  tick();
  setInterval(tick, 30000);

  /* ---------- prism flip: auto-roll + scroll-wheel + keyboard ---------- */
  var minibar = document.querySelector("[data-minibar]");
  var prism = document.querySelector("[data-mb-prism]");
  var flipTimer = null;

  function setFaces() {
    if (!prism) return;
    var flipped = prism.classList.contains("flipped");
    var claude = prism.querySelector(".mb-face--claude");
    var codex = prism.querySelector(".mb-face--codex");
    if (claude) claude.setAttribute("aria-hidden", flipped ? "true" : "false");
    if (codex) codex.setAttribute("aria-hidden", flipped ? "false" : "true");
  }

  function flip() {
    if (!prism) return;
    prism.classList.toggle("flipped");
    setFaces();
  }

  function startAutoFlip() {
    if (reducedMotion || flipTimer) return;
    flipTimer = setInterval(flip, 4500);
  }
  function stopAutoFlip() {
    clearInterval(flipTimer);
    flipTimer = null;
  }

  if (minibar && prism) {
    startAutoFlip();
    minibar.addEventListener("mouseenter", stopAutoFlip);
    minibar.addEventListener("mouseleave", startAutoFlip);
    minibar.addEventListener("wheel", function (e) {
      e.preventDefault();
      flip();
    }, { passive: false });
    minibar.addEventListener("keydown", function (e) {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        flip();
      }
    });
    minibar.addEventListener("click", flip);
  }

  /* ---------- anatomy status-light chips ---------- */
  var anatLight = document.querySelector("[data-anat-light]");
  var heroLight = document.querySelector("[data-mb-light]");
  document.querySelectorAll("[data-set-state]").forEach(function (btn) {
    btn.addEventListener("click", function () {
      var state = btn.dataset.setState;
      if (anatLight) anatLight.dataset.state = state;
      if (heroLight) heroLight.dataset.state = state;
      document.querySelectorAll("[data-set-state]").forEach(function (b) {
        b.setAttribute("aria-pressed", b === btn ? "true" : "false");
      });
    });
  });

  /* ---------- download dropdown ---------- */
  var dl = document.querySelector("[data-dl]");
  var dlBtn = document.querySelector("[data-dl-btn]");
  var dlMenu = document.querySelector("[data-dl-menu]");

  function closeMenu() {
    if (!dlMenu || dlMenu.hidden) return;
    dlMenu.hidden = true;
    dlBtn.setAttribute("aria-expanded", "false");
  }

  if (dl && dlBtn && dlMenu) {
    dlBtn.addEventListener("click", function () {
      var open = !dlMenu.hidden;
      dlMenu.hidden = open;
      dlBtn.setAttribute("aria-expanded", String(!open));
      if (!open) {
        var first = dlMenu.querySelector("a");
        if (first) first.focus();
      }
    });
    document.addEventListener("click", function (e) {
      if (!dl.contains(e.target)) closeMenu();
    });
    document.addEventListener("keydown", function (e) {
      if (e.key === "Escape") {
        closeMenu();
        dlBtn.focus();
      }
    });
    dlMenu.addEventListener("keydown", function (e) {
      var items = Array.prototype.slice.call(dlMenu.querySelectorAll("a"));
      var idx = items.indexOf(document.activeElement);
      if (e.key === "ArrowDown") {
        e.preventDefault();
        items[(idx + 1) % items.length].focus();
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        items[(idx - 1 + items.length) % items.length].focus();
      }
    });
  }

  /* ---------- reveal on scroll ---------- */
  var targets = document.querySelectorAll(".reveal, [data-animate], .mp-trend");
  if ("IntersectionObserver" in window && !reducedMotion) {
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (entry.isIntersecting) {
          entry.target.classList.add("in");
          io.unobserve(entry.target);
        }
      });
    }, { threshold: 0.25, rootMargin: "0px 0px -8% 0px" });
    targets.forEach(function (t) { io.observe(t); });
  } else {
    targets.forEach(function (t) { t.classList.add("in"); });
  }
})();
