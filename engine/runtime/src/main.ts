// Presentation runtime for s-md-slides HTML exports (FR-EXP-02, FR-ANIM, FR-DIR-02).
// It reads the timeline the engine wrote into #smd-data and plays it with the same
// ease-out cubic blend as the Rust `state_at`, so HTML, GIF and hosts match.

type State = { opacity: number; dx: number; dy: number; scale: number };
type Entry = { id: string; delay: number; dur: number; from: State; to: State };
type Step = { start: number; duration: number; entries: Entry[] };
type Timeline = { initial: Record<string, State>; steps: Step[]; duration: number };
type SlideData = { transition: string; dur: number; notes: string; timeline: Timeline };
type Data = { title: string; width: number; height: number; rtl: boolean; slides: SlideData[] };
type Nav = { slide: number; step: number; ended: boolean };

const $ = <T extends HTMLElement>(sel: string) => document.querySelector(sel) as T | null;
const data: Data = JSON.parse($("#smd-data")?.textContent || "{}");
const stage = $("#smd-stage")!;
const sections = Array.from(stage.querySelectorAll<HTMLElement>("section.smd-slide"));
const endScreen = $("#smd-end")!;
const n = sections.length;
const presenterMode = location.hash === "#presenter";
const reduced = matchMedia("(prefers-reduced-motion: reduce)");
const pristine = sections.map((s) => s.cloneNode(true) as HTMLElement);

const ease = (p: number) => 1 - Math.pow(1 - Math.min(1, Math.max(0, p)), 3);
const mix = (a: State, b: State, p: number): State => ({
  opacity: a.opacity + (b.opacity - a.opacity) * p,
  dx: a.dx + (b.dx - a.dx) * p,
  dy: a.dy + (b.dy - a.dy) * p,
  scale: a.scale + (b.scale - a.scale) * p,
});

// Each element's own rotation and opacity, as drawn at its end state.
type Box = { el: HTMLElement; rotate: string; opacity: number };
const boxes: Map<string, Box>[] = sections.map((s) => {
  const m = new Map<string, Box>();
  s.querySelectorAll<HTMLElement>(".smd-el[data-id]:not([data-master])").forEach((el) => {
    const rotate = (el.style.transform.match(/rotate\([^)]*\)/) || [""])[0];
    m.set(el.dataset.id!, { el, rotate, opacity: parseFloat(el.style.opacity || "1") });
  });
  return m;
});

function paint(slide: number, id: string, s: State) {
  const b = boxes[slide].get(id);
  if (!b) return;
  const parts = [];
  if (s.dx || s.dy) parts.push(`translate(${s.dx}px,${s.dy}px)`);
  if (b.rotate) parts.push(b.rotate);
  if (s.scale !== 1) parts.push(`scale(${s.scale})`);
  b.el.style.transform = parts.join(" ");
  b.el.style.opacity = String(b.opacity * s.opacity);
}

/** States after steps 0..=k have finished. */
function statesAfter(slide: number, k: number): Record<string, State> {
  const t = data.slides[slide].timeline;
  const out: Record<string, State> = { ...t.initial };
  t.steps.slice(0, k + 1).forEach((st) => st.entries.forEach((e) => (out[e.id] = e.to)));
  return out;
}

function show(slide: number, states: Record<string, State>) {
  for (const id of Object.keys(states)) paint(slide, id, states[id]);
}

// Animations run on one requestAnimationFrame loop; `finish` jumps them to the end.
type Run = { slide: number; e: Entry; t0: number };
let runs: Run[] = [];
let tweens: { t0: number; dur: number; apply: (p: number) => void; done?: () => void }[] = [];
let frame = 0;

function tick(now: number) {
  runs = runs.filter((r) => {
    const t = now - r.t0 - r.e.delay;
    if (t < 0) return true;
    const p = r.e.dur ? t / r.e.dur : 1;
    paint(r.slide, r.e.id, mix(r.e.from, r.e.to, ease(p)));
    return p < 1;
  });
  tweens = tweens.filter((w) => {
    const p = w.dur ? (now - w.t0) / w.dur : 1;
    w.apply(ease(p));
    if (p >= 1) w.done?.();
    return p < 1;
  });
  frame = runs.length || tweens.length ? requestAnimationFrame(tick) : 0;
}

function start() {
  if (!frame) frame = requestAnimationFrame(tick);
}

function finish() {
  for (const r of runs) paint(r.slide, r.e.id, r.e.to);
  for (const w of tweens) {
    w.apply(1);
    w.done?.();
  }
  runs = [];
  tweens = [];
}

function playStep(slide: number, k: number) {
  const st = data.slides[slide].timeline.steps[k];
  if (!st) return;
  if (reduced.matches) {
    st.entries.forEach((e) => paint(slide, e.id, e.to));
    return;
  }
  const t0 = performance.now();
  st.entries.forEach((e) => {
    paint(slide, e.id, e.from);
    runs.push({ slide, e, t0 });
  });
  start();
}

const clicks = (slide: number) => Math.max(0, data.slides[slide].timeline.steps.length - 1);

let cur: Nav = { slide: 0, step: 0, ended: false };

function pauseMedia(i: number) {
  sections[i]?.querySelectorAll("video").forEach((v) => v.pause());
}

function chrome() {
  const counter = $("#smd-counter");
  if (counter) counter.textContent = cur.ended ? "End" : `${cur.slide + 1} / ${n}`;
  const bar = $("#smd-progress");
  if (bar) {
    const done = cur.ended ? n : cur.slide + 1;
    bar.setAttribute("aria-valuenow", String(done));
    (bar.firstElementChild as HTMLElement).style.width = `${(done / Math.max(1, n)) * 100}%`;
  }
  if (!presenterMode && history.replaceState) history.replaceState(null, "", `#${cur.slide + 1}`);
  if (presenterMode) presenter.update();
}

/** Put the deck at `to` without animation, except step 0 and the transition. */
function go(to: Nav, animate: boolean, send = true) {
  finish();
  const from = cur;
  cur = to;
  endScreen.hidden = !to.ended;
  sections.forEach((s, i) => {
    s.style.display = !to.ended && i === to.slide ? "" : "none";
    s.style.transform = "";
    s.style.opacity = "";
  });
  if (from.slide !== to.slide) pauseMedia(from.slide);
  if (!to.ended) {
    const fresh = animate && from.slide !== to.slide && to.step === 0;
    show(to.slide, fresh ? data.slides[to.slide].timeline.initial : statesAfter(to.slide, to.step));
    if (fresh) {
      transition(from, to);
      playStep(to.slide, 0);
    }
  }
  chrome();
  if (send) channel?.postMessage(cur);
}

function transition(from: Nav, to: Nav) {
  const t = data.slides[to.slide];
  if (reduced.matches || from.ended || t.transition === "none" || !t.dur) return;
  const a = sections[from.slide];
  const b = sections[to.slide];
  const forward = to.slide > from.slide;
  a.style.display = "";
  if (t.transition === "push") {
    // Push follows reading direction: in an rtl deck the next slide comes from the left.
    const dir = (forward ? -1 : 1) * (data.rtl ? -1 : 1) * data.width;
    tweens.push({
      t0: performance.now(),
      dur: t.dur,
      apply: (p) => {
        a.style.transform = `translateX(${dir * p}px)`;
        b.style.transform = `translateX(${-dir * (1 - p)}px)`;
      },
      done: () => {
        a.style.display = "none";
        a.style.transform = b.style.transform = "";
      },
    });
  } else {
    b.style.opacity = "0";
    tweens.push({
      t0: performance.now(),
      dur: t.dur,
      apply: (p) => (b.style.opacity = String(p)),
      done: () => {
        a.style.display = "none";
        b.style.opacity = "";
      },
    });
  }
  start();
}

function next() {
  if (cur.ended) return;
  finish();
  if (cur.step < clicks(cur.slide)) {
    cur = { ...cur, step: cur.step + 1 };
    playStep(cur.slide, cur.step);
    chrome();
    channel?.postMessage(cur);
  } else if (cur.slide < n - 1) go({ slide: cur.slide + 1, step: 0, ended: false }, true);
  else go({ ...cur, ended: true }, false);
}

function prev() {
  if (cur.ended) return go({ ...cur, ended: false }, false);
  if (cur.step > 0) return go({ ...cur, step: cur.step - 1 }, false);
  if (cur.slide > 0) go({ slide: cur.slide - 1, step: clicks(cur.slide - 1), ended: false }, false);
}

function first() {
  go({ slide: 0, step: 0, ended: false }, true);
}

function last() {
  go({ slide: n - 1, step: clicks(n - 1), ended: false }, false);
}

function fullscreen() {
  if (document.fullscreenElement) document.exitFullscreen?.();
  else document.documentElement.requestFullscreen?.().catch(() => {});
}

function openPresenter() {
  window.open(location.href.split("#")[0] + "#presenter", "smd-presenter", "width=1200,height=760");
}

// Keyboard: in an rtl deck Left and Right swap (FR-DIR-02). Positions never mirror.
addEventListener("keydown", (ev) => {
  if (ev.ctrlKey || ev.metaKey || ev.altKey) return;
  const target = ev.target as HTMLElement;
  if (target && (target.isContentEditable || /^(INPUT|TEXTAREA|SELECT|VIDEO|IFRAME)$/.test(target.tagName))) return;
  let key = ev.key;
  if (data.rtl && key === "ArrowLeft") key = "ArrowRight";
  else if (data.rtl && key === "ArrowRight") key = "ArrowLeft";
  const act: Record<string, () => void> = {
    ArrowRight: next,
    ArrowDown: next,
    PageDown: next,
    " ": next,
    ArrowLeft: prev,
    ArrowUp: prev,
    PageUp: prev,
    Home: first,
    End: last,
    f: fullscreen,
    F: fullscreen,
    p: openPresenter,
    P: openPresenter,
  };
  const fn = act[key];
  if (!fn) return;
  if (key === " " && target?.tagName === "BUTTON") return;
  ev.preventDefault();
  fn();
});

// Touch: swipe toward the reading direction's start for the next slide.
let touch: { x: number; y: number } | null = null;
stage.addEventListener("touchstart", (e) => (touch = { x: e.touches[0].clientX, y: e.touches[0].clientY }), { passive: true });
stage.addEventListener("touchend", (e) => {
  if (!touch) return;
  const dx = e.changedTouches[0].clientX - touch.x;
  const dy = e.changedTouches[0].clientY - touch.y;
  touch = null;
  if (Math.abs(dx) < 50 || Math.abs(dx) < Math.abs(dy)) return;
  if (dx < 0 !== data.rtl) next();
  else prev();
});

$("#smd-next")?.addEventListener("click", next);
$("#smd-prev")?.addEventListener("click", prev);
$("#smd-full")?.addEventListener("click", fullscreen);
$("#smd-presenter")?.addEventListener("click", openPresenter);

// Fit the canvas to its box, letterboxed.
function fit(el: HTMLElement, box: { width: number; height: number }) {
  const s = Math.min(box.width / data.width, box.height / data.height);
  el.style.width = `${data.width}px`;
  el.style.height = `${data.height}px`;
  el.style.transform = `translate(-50%,-50%) scale(${s})`;
}

// Presenter view: current slide, next slide, notes and a timer (BroadcastChannel sync).
const channel: BroadcastChannel | null =
  "BroadcastChannel" in window ? new BroadcastChannel(`smd-${data.title}`) : null;
channel?.addEventListener("message", (m: MessageEvent<Nav>) => go(m.data, false, false));

const presenter = {
  next: null as HTMLElement | null,
  notes: null as HTMLElement | null,
  t0: Date.now(),
  init() {
    document.body.classList.add("smd-presenter-mode");
    const wrap = document.createElement("div");
    wrap.className = "smd-pv";
    wrap.innerHTML =
      '<div class="smd-pv-cur"></div><div class="smd-pv-side"><div class="smd-pv-label">Next</div>' +
      '<div class="smd-pv-next"><div class="smd-pv-stage"></div></div><div class="smd-pv-label">Notes</div>' +
      '<div class="smd-pv-notes"></div><div class="smd-pv-timer"><span>00:00</span>' +
      '<button type="button" aria-label="Restart timer">Reset</button></div></div>';
    document.body.appendChild(wrap);
    wrap.querySelector(".smd-pv-cur")!.appendChild(stage);
    this.next = wrap.querySelector(".smd-pv-stage");
    this.notes = wrap.querySelector(".smd-pv-notes");
    const time = wrap.querySelector(".smd-pv-timer span")!;
    wrap.querySelector(".smd-pv-timer button")!.addEventListener("click", () => (this.t0 = Date.now()));
    setInterval(() => {
      const s = Math.floor((Date.now() - this.t0) / 1000);
      time.textContent = `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
    }, 500);
  },
  update() {
    if (!this.next || !this.notes) return;
    const i = Math.min(cur.slide + 1, n - 1);
    this.next.replaceChildren();
    if (cur.slide + 1 < n) this.next.appendChild(pristine[i].cloneNode(true));
    else this.next.textContent = "End of deck";
    // Notes are already sanitized by the engine (FR-EXP-07).
    this.notes.innerHTML = data.slides[cur.slide]?.notes || "<p>No notes.</p>";
    layout();
  },
};

function layout() {
  if (presenterMode) {
    const cur = stage.parentElement!.getBoundingClientRect();
    fit(stage, cur);
    const nx = presenter.next;
    if (nx) fit(nx, nx.parentElement!.getBoundingClientRect());
  } else fit(stage, { width: innerWidth, height: innerHeight });
}

addEventListener("resize", layout);
if (presenterMode) presenter.init();

const fromHash = parseInt(location.hash.slice(1), 10);
const startAt = Number.isFinite(fromHash) ? Math.min(Math.max(fromHash, 1), n) - 1 : 0;
cur = { slide: startAt, step: 0, ended: n === 0 };
go({ slide: startAt, step: 0, ended: n === 0 }, false, false);
playStep(startAt, 0);
layout();
const ready = () => document.body.classList.remove("smd-loading");
(document.fonts?.ready ?? Promise.resolve()).then(ready, ready);

// Hooks for `serve` reloads and tests.
(window as unknown as { smd: object }).smd = {
  next,
  prev,
  go: (slide: number, step = 0) => go({ slide, step, ended: false }, false),
  state: () => ({ ...cur }),
};
