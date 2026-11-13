import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import "./styles.css";
import "./workflow.css";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  FolderOpenIcon,
  FileDownloadIcon,
  InformationCircleIcon,
  Cancel01Icon,
  CdIcon,
} from "@hugeicons/core-free-icons";

type AppError = {
  category: string;
  message: string;
  recovery: string;
  detail: string;
};
type ImageInfo = {
  path: string;
  name: string;
  size: number;
  totalBytes: number;
  totalFiles: number;
  hasXex: boolean;
  suggestedName: string;
};
type Progress = {
  bytesDone: number;
  totalBytes: number;
  filesDone: number;
  totalFiles: number;
  currentFile: string;
  elapsedSeconds: number;
};
type QueueItem = {
  image: ImageInfo;
  status: string;
  result: {
    path: string;
    bytes: number;
    files: number;
    hasXex: boolean;
  } | null;
};
type OutputFormat = "folder" | "god";
type Snapshot = {
  outputFormat: OutputFormat;
  queue: QueueItem[];
  status:
    | "idle"
    | "validating"
    | "ready"
    | "extracting"
    | "completed"
    | "failed"
    | "cancelled";
  image: ImageInfo | null;
  progress: Progress;
  destination: string | null;
  partialPath: string | null;
  result: {
    path: string;
    bytes: number;
    files: number;
    hasXex: boolean;
  } | null;
  error: AppError | null;
  logs: string[];
};
const empty: Snapshot = {
  outputFormat: "folder",
  queue: [],
  status: "idle",
  image: null,
  destination: null,
  partialPath: null,
  result: null,
  error: null,
  logs: [],
  progress: {
    bytesDone: 0,
    totalBytes: 0,
    filesDone: 0,
    totalFiles: 0,
    currentFile: "",
    elapsedSeconds: 0,
  },
};
const native = "__TAURI_INTERNALS__" in window;
const bytes = (value: number) => {
  if (!value) return "0 B";
  const unit = Math.min(Math.floor(Math.log(value) / Math.log(1024)), 4);
  return `${(value / 1024 ** unit).toLocaleString(undefined, { maximumFractionDigits: unit ? 2 : 0 })} ${["B", "KiB", "MiB", "GiB", "TiB"][unit]}`;
};
const elapsed = (seconds: number) =>
  `${Math.floor(seconds / 60)
    .toString()
    .padStart(2, "0")}:${(seconds % 60).toString().padStart(2, "0")}`;
function Icon({ name, size = 22 }: { name: string; size?: number }) {
  const paths: Record<string, React.ReactNode> = {
    disc: (
      <>
        <circle cx="12" cy="12" r="9" />
        <circle cx="12" cy="12" r="2" />
        <path d="M15 5.8a7 7 0 0 1 3.2 3.2M16 9a4 4 0 0 0-1-1" />
      </>
    ),
    upload: (
      <>
        <path d="M12 16V4m-5 5 5-5 5 5M4 16v4h16v-4" />
      </>
    ),
    folder: <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v10H3Z" />,
    check: (
      <>
        <circle cx="12" cy="12" r="9" />
        <path d="m8 12 3 3 5-6" />
      </>
    ),
    arrow: <path d="M5 12h14m-5-5 5 5-5 5" />,
    warning: (
      <>
        <path d="m12 3 10 18H2Z" />
        <path d="M12 9v5m0 3v.1" />
      </>
    ),
    lock: (
      <>
        <rect x="5" y="10" width="14" height="11" rx="2" />
        <path d="M8 10V7a4 4 0 0 1 8 0v3m-4 5v2" />
      </>
    ),
    copy: (
      <>
        <rect x="8" y="8" width="12" height="13" rx="2" />
        <path d="M15 8V3H3v12h5" />
      </>
    ),
  };
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.65"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {paths[name] ?? paths.disc}
    </svg>
  );
}
function AmbientOcean() {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d", { alpha: true });
    if (!canvas || !context) return;

    const motion = window.matchMedia("(prefers-reduced-motion: reduce)");
    let frame = 0;
    let lastFrame = 0;
    let width = 0;
    let height = 0;
    let pixelRatio = 1;
    const startedAt = performance.now();

    function resize() {
      width = window.innerWidth;
      height = window.innerHeight;
      pixelRatio = Math.min(window.devicePixelRatio || 1, 1.5);
      canvas!.width = Math.round(width * pixelRatio);
      canvas!.height = Math.round(height * pixelRatio);
      context!.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
      draw(motion.matches ? 0 : performance.now() - startedAt);
    }

    function draw(time: number) {
      context!.clearRect(0, 0, width, height);

      const glow = context!.createRadialGradient(
        width * 0.72,
        height * 0.5,
        0,
        width * 0.72,
        height * 0.5,
        Math.max(width, height) * 0.7,
      );
      glow.addColorStop(0, "rgba(80, 151, 52, 0.075)");
      glow.addColorStop(0.5, "rgba(44, 107, 41, 0.035)");
      glow.addColorStop(1, "rgba(12, 29, 17, 0)");
      context!.fillStyle = glow;
      context!.fillRect(0, 0, width, height);

      const clamp = (value: number) => Math.min(1, Math.max(0, value));
      const columns = Math.ceil(width / 23) + 2;
      const rows = 27;
      const uAt = (column: number) =>
        -1.2 + (2.4 * column) / Math.max(1, columns - 1);
      const vAt = (row: number) => (1.18 * row) / Math.max(1, rows - 1);

      function surface(u: number, v: number) {
        // A visible rise and fall within a few seconds, rather than the old
        // minute-long swells that made the background appear frozen.
        const phaseA = u * 4.25 - v * 2.65 - time * 0.00042;
        const phaseB = u * 2.05 + v * 3.5 + time * 0.00027;
        const depth = v / 1.18;
        const heightScale = height * (0.012 + depth * 0.021);
        const swell =
          Math.sin(phaseA) * heightScale +
          Math.sin(phaseB) * heightScale * 0.42;
        const x = width * 0.5 + u * width * 0.59 + v * width * 0.115;
        const y = height * 0.11 + v * height * 0.82 + u * height * 0.09 + swell;
        const edgeFade = clamp(
          Math.min(x / (width * 0.16), (width - x) / (width * 0.16)),
        );
        const horizonFade = clamp((depth - 0.015) / 0.28);
        return { x, y, depth, phase: phaseA, edgeFade, horizonFade };
      }

      // Receding cross-contours give the surface its isometric plane.
      for (let row = 0; row < rows; row += 1) {
        const depth = row / Math.max(1, rows - 1);
        const emphasized = row % 5 === 0;
        context!.beginPath();
        for (let column = 0; column < columns; column += 1) {
          const point = surface(uAt(column), vAt(row));
          if (column === 0) context!.moveTo(point.x, point.y);
          else context!.lineTo(point.x, point.y);
        }
        context!.strokeStyle = `rgba(147, 221, 111, ${
          (emphasized ? 0.035 : 0.014) + depth * (emphasized ? 0.09 : 0.045)
        })`;
        context!.lineWidth = emphasized ? 0.9 : 0.55;
        context!.stroke();
      }

      // Longitudinal lines bend across each swell to make the wave height legible.
      for (let column = 0; column < columns; column += 1) {
        const emphasized = column % 5 === 0;
        context!.beginPath();
        for (let row = 0; row < rows; row += 1) {
          const point = surface(uAt(column), vAt(row));
          if (row === 0) context!.moveTo(point.x, point.y);
          else context!.lineTo(point.x, point.y);
        }
        const depth = 0.52;
        context!.strokeStyle = `rgba(150, 226, 114, ${
          (emphasized ? 0.038 : 0.012) + depth * (emphasized ? 0.082 : 0.038)
        })`;
        context!.lineWidth = emphasized ? 0.85 : 0.5;
        context!.stroke();
      }

      // Tiny lights sit at the mesh intersections like reflections on the water.
      for (let row = 1; row < rows; row += 1) {
        for (let column = 0; column < columns; column += 1) {
          const point = surface(uAt(column), vAt(row));
          const shimmer =
            0.82 +
            0.18 * (0.5 + 0.5 * Math.sin(point.phase * 0.6 + row * 0.27));
          const alpha =
            (0.035 + point.depth * 0.12) *
            point.horizonFade *
            point.edgeFade *
            shimmer;
          const radius = 0.5 + point.depth * 0.78;

          context!.beginPath();
          context!.arc(point.x, point.y, radius, 0, Math.PI * 2);
          context!.fillStyle = `rgba(190, 238, 142, ${alpha})`;
          context!.fill();

          if (
            point.depth > 0.4 &&
            (column * 17 + row * 29) % 61 === 0 &&
            Math.sin(point.phase) > 0.68
          ) {
            context!.beginPath();
            context!.arc(point.x, point.y, radius + 0.75, 0, Math.PI * 2);
            context!.fillStyle = `rgba(224, 255, 183, ${alpha * 0.78})`;
            context!.fill();
          }
        }
      }
    }

    function animate(now: number) {
      if (motion.matches) return;
      if (document.visibilityState !== "hidden" && now - lastFrame >= 40) {
        draw(now - startedAt);
        lastFrame = now;
      }
      // WebKit can briefly report a foreground window as hidden during app
      // activation. Keep the scheduler alive so the next visible frame recovers.
      frame = window.requestAnimationFrame(animate);
    }

    function start() {
      window.cancelAnimationFrame(frame);
      if (motion.matches) draw(0);
      else frame = window.requestAnimationFrame(animate);
    }

    function onVisibilityChange() {
      if (document.visibilityState === "hidden")
        window.cancelAnimationFrame(frame);
      else start();
    }

    resize();
    start();
    window.addEventListener("resize", resize);
    window.addEventListener("focus", start);
    document.addEventListener("visibilitychange", onVisibilityChange);
    motion.addEventListener("change", start);

    return () => {
      window.cancelAnimationFrame(frame);
      window.removeEventListener("resize", resize);
      window.removeEventListener("focus", start);
      document.removeEventListener("visibilitychange", onVisibilityChange);
      motion.removeEventListener("change", start);
    };
  }, []);

  return (
    <canvas ref={canvasRef} className="ambient-ocean" aria-hidden="true" />
  );
}
function App() {
  const detailsRef = useRef<HTMLDialogElement>(null);
  function openDetails() {
    const dialog = detailsRef.current;
    if (!dialog || dialog.open) return;
    dialog.showModal();
    dialog
      .querySelector<HTMLElement>("#sheet-title")
      ?.focus({ preventScroll: true });
  }
  function closeDetails() {
    const dialog = detailsRef.current;
    if (!dialog?.open) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      dialog.close();
      return;
    }
    const animation = dialog.animate(
      [
        { transform: "translateX(0)", opacity: 1 },
        { transform: "translateX(24px)", opacity: 0 },
      ],
      { duration: 160, easing: "ease-out" },
    );
    animation.onfinish = () => dialog.close();
  }
  const [s, setS] = useState<Snapshot>(empty);
  const [format, setFormat] = useState<OutputFormat>("folder");
  const surfaceRef = useRef<HTMLElement>(null);
  const firstMode = useRef(true);
  useLayoutEffect(() => {
    document.documentElement.dataset.format = format;
    if (firstMode.current) {
      firstMode.current = false;
      return;
    }
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const animations: Animation[] = [];
    surfaceRef.current
      ?.querySelectorAll(".mode-title, .mode-route")
      .forEach((element, index) => {
        animations.push(
          element.animate(
            [
              {
                opacity: 0,
                transform: "translateY(10px)",
              },
              { opacity: 1, transform: "translateY(0)" },
            ],
            {
              duration: 440,
              delay: index * 35,
              easing: "cubic-bezier(0.22, 1, 0.36, 1)",
              fill: "backwards",
            },
          ),
        );
      });
    return () => animations.forEach((animation) => animation.cancel());
  }, [format]);
  const [name, setName] = useState("");
  const [localError, setLocalError] = useState<AppError | null>(null);
  const [dragging, setDragging] = useState(false);
  const [pending, setPending] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [notice, setNotice] = useState("");
  const stateRef = useRef(s);
  stateRef.current = s;
  const busy =
    pending || s.status === "validating" || s.status === "extracting";
  const locked = busy || !!s.partialPath;
  const multiple = s.queue.length > 1;
  const completed =
    s.outputFormat === format
      ? s.queue.filter((item) => item.result).length
      : 0;
  const selectionLocked =
    locked || s.queue.some((item) => item.result !== null);
  const error = localError ?? s.error;
  const percent = s.progress.totalBytes
    ? Math.floor((100 * s.progress.bytesDone) / s.progress.totalBytes)
    : s.progress.totalFiles
      ? Math.floor((100 * s.progress.filesDone) / s.progress.totalFiles)
      : 0;
  async function call<T>(
    command: string,
    args?: Record<string, unknown>,
  ): Promise<T | undefined> {
    setLocalError(null);
    setNotice("");
    try {
      return await invoke<T>(command, args);
    } catch (e) {
      setLocalError(
        typeof e === "object" && e !== null && "message" in e
          ? (e as AppError)
          : {
              category: "app",
              message: "The operation could not be completed.",
              recovery: "Try again. Technical details are available below.",
              detail: String(e),
            },
      );
    }
  }
  async function chooseIso() {
    if (selectionLocked) return;
    setPending(true);
    await call("choose_iso", { append: s.queue.length > 0 });
    setPending(false);
  }
  async function removeImage(index: number) {
    if (selectionLocked) return;
    const paths = s.queue
      .filter((_, i) => i !== index)
      .map((item) => item.image.path);
    setPending(true);
    if (paths.length) await call("inspect_drop", { paths });
    else await call("reset");
    setPending(false);
  }
  async function chooseFolder() {
    setPending(true);
    const path = await call<string | null>("choose_folder");
    if (path && name) {
      const suggestion = await call<string>("suggest_name", { name });
      if (suggestion) setName(suggestion);
    }
    setPending(false);
  }
  async function start() {
    setPending(true);
    setCancelling(false);
    await call("start_extraction", { name: name.trim(), format });
    setPending(false);
  }
  async function resolve(deleteFiles: boolean) {
    setPending(true);
    await call("resolve_partial", { delete: deleteFiles });
    setPending(false);
  }
  async function copyDetails() {
    // Full host paths and absolute destination names are intentionally excluded.
    const report = [
      "Xiso Harbor 1.1.0 / xdvdfs 0.8.3 / arm64",
      `State: ${s.status}`,
      `Category: ${error?.category ?? "none"}`,
      error?.message ?? "",
      `Files: ${s.progress.filesDone}/${s.progress.totalFiles}`,
      `Bytes: ${s.progress.bytesDone}/${s.progress.totalBytes}`,
      error?.detail ?? "",
      ...s.logs,
    ].join("\n");
    try {
      if (native) await invoke("copy_diagnostics", { error });
      else await navigator.clipboard.writeText(report);
      setNotice("Diagnostic details copied. Review them before sharing.");
    } catch {
      setNotice("Copy was unavailable. Select and copy the details below.");
    }
  }
  useEffect(() => {
    if (!native) return;
    let disposed = false;
    const unlisteners: (() => void)[] = [];
    const register = async () => {
      const unlisten = await listen<Snapshot>("extraction-state", (e) => {
        if (!disposed) setS(e.payload);
      });
      if (disposed) unlisten();
      else unlisteners.push(unlisten);
      const close = await listen("close-blocked", () =>
        setNotice(
          "An operation is running. Cancel extraction and wait for it to stop before closing.",
        ),
      );
      if (disposed) close();
      else unlisteners.push(close);
      const drop = await getCurrentWebview().onDragDropEvent(async (e) => {
        if (e.payload.type === "over" || e.payload.type === "enter")
          setDragging(true);
        else setDragging(false);
        if (
          e.payload.type === "drop" &&
          !["validating", "extracting"].includes(stateRef.current.status) &&
          !stateRef.current.partialPath &&
          !stateRef.current.queue.some((item) => item.result)
        ) {
          setPending(true);
          await call("inspect_drop", {
            paths: [
              ...stateRef.current.queue.map((item) => item.image.path),
              ...e.payload.paths,
            ],
          });
          setPending(false);
        }
      });
      if (disposed) drop();
      else unlisteners.push(drop);
      const current = await invoke<Snapshot>("get_state");
      if (!disposed) setS(current);
    };
    register().catch((e) =>
      setLocalError({
        category: "startup",
        message: "The native app connection could not start.",
        recovery: "Close the app and open it again.",
        detail: String(e),
      }),
    );
    const timer = window.setInterval(() => {
      if (["validating", "extracting"].includes(stateRef.current.status))
        invoke<Snapshot>("get_state")
          .then((v) => {
            if (!disposed) setS(v);
          })
          .catch(() => {});
    }, 1000);
    return () => {
      disposed = true;
      window.clearInterval(timer);
      unlisteners.forEach((f) => f());
    };
  }, []);
  useEffect(() => {
    if (s.image) {
      const desired = s.image.suggestedName;
      if (native)
        invoke<string>("suggest_name", { name: desired })
          .then(setName)
          .catch(() => setName(desired));
      else setName(desired);
    } else setName("");
  }, [s.image?.path]);
  return (
    <>
      <AmbientOcean />
      <header className="app-header">
        <div className="app-icon">
          <Icon name="disc" size={29} />
        </div>
        <h1>
          Xiso Harbor
          <span className="wordmark-caption">Extract your Xbox games</span>
        </h1>
        <button
          className="details-trigger"
          onClick={openDetails}
          aria-label="Open details"
          aria-haspopup="dialog"
        >
          <HugeiconsIcon
            icon={InformationCircleIcon}
            size={18}
            aria-hidden="true"
          />
          Help &amp; details
        </button>
      </header>
      <main
        ref={surfaceRef}
        className={`${busy ? "is-working" : ""} ${s.status === "completed" ? "is-complete" : ""}`}
      >
        {!native && (
          <div className="alert" role="alert">
            This is the browser view. Open the installed macOS app to choose
            files and extract images.
          </div>
        )}
        {s.status !== "completed" && (
          <section className="card format-card" aria-label="Output format">
            <div className="section-label">
              <span className="step">01</span>
              <h2>Output format</h2>
            </div>
            <div className="mode-heading">
              <div className="mode-heading-copy">
                <h2 className="mode-title">
                  {format === "folder" ? "Extracted files" : "On demand"}
                  <span>.</span>
                </h2>
                <p className="mode-route">
                  {format === "folder"
                    ? "ISO → Extracted files"
                    : "ISO → GOD package"}
                </p>
              </div>
              <div className="disc-motif" aria-hidden="true">
                <div className="disc-engraving">
                  <span className="disc-groove" />
                  <span className="disc-hub" />
                </div>
              </div>
            </div>
            <fieldset className="format-options" disabled={locked}>
              <legend className="sr-only">Choose output format</legend>
              <label className={format === "folder" ? "selected" : ""}>
                <input
                  type="radio"
                  name="format"
                  value="folder"
                  checked={format === "folder"}
                  onChange={() => setFormat("folder")}
                />
                <span className="format-name">
                  Extracted files{" "}
                  <small>For Aurora &amp; other launchers</small>
                </span>
              </label>
              <label className={format === "god" ? "selected" : ""}>
                <input
                  type="radio"
                  name="format"
                  value="god"
                  checked={format === "god"}
                  onChange={() => setFormat("god")}
                />
                <span className="format-name">
                  Games on Demand <small>For the Xbox dashboard</small>
                </span>
              </label>
            </fieldset>
          </section>
        )}
        {(s.status !== "completed" || s.queue.length > 0) && (
          <>
            <section
              className="card games-region"
              aria-labelledby="games-heading"
            >
              <div className="section-label">
                <span className="step">02</span>
                <h2 id="games-heading">Games</h2>
                <span className="section-hint">
                  {format === "god"
                    ? "Xbox 360 · single-disc"
                    : "Xbox / Xbox 360"}
                </span>
              </div>
              {s.queue.length === 0 ? (
                <div
                  className={`drop-zone ${dragging && !locked ? "dragging" : ""}`}
                >
                  <div className="drop-icon">
                    <HugeiconsIcon icon={CdIcon} size={36} aria-hidden="true" />
                  </div>
                  <div className="drop-description">
                    <h3>
                      {s.status === "validating"
                        ? "Checking game images…"
                        : "Drop your game images here"}
                    </h3>
                    <p>ISO or XISO · One game or a whole queue</p>
                  </div>
                  <button
                    className="secondary"
                    onClick={chooseIso}
                    disabled={selectionLocked || !native}
                  >
                    <HugeiconsIcon icon={CdIcon} size={18} aria-hidden="true" />
                    Browse games
                  </button>
                </div>
              ) : (
                <>
                  <div className="games-toolbar">
                    <span>
                      {s.queue.length}{" "}
                      {multiple ? "games selected" : "game selected"}
                    </span>
                    <button
                      className="secondary"
                      onClick={chooseIso}
                      disabled={selectionLocked || !native}
                    >
                      <HugeiconsIcon
                        icon={CdIcon}
                        size={18}
                        aria-hidden="true"
                      />
                      Add games
                    </button>
                  </div>
                  <ol className="selected-games">
                    {s.queue.map((item, index) => (
                      <li
                        key={item.image.path}
                        aria-current={
                          busy && s.image?.path === item.image.path
                            ? "step"
                            : undefined
                        }
                      >
                        <HugeiconsIcon
                          icon={CdIcon}
                          size={24}
                          aria-hidden="true"
                        />
                        <div className="selected-game-copy">
                          <strong title={item.image.name}>
                            {item.image.name}
                          </strong>
                          <span>
                            {item.image.totalFiles.toLocaleString()} files ·{" "}
                            {bytes(item.image.totalBytes)}
                          </span>
                        </div>
                        <span className="game-state">
                          {s.outputFormat === format && item.result
                            ? "Completed"
                            : busy && item.status === "extracting"
                              ? "Processing"
                              : item.status === "failed"
                                ? "Needs attention"
                                : "Image checked"}
                        </span>
                        {item.result && s.outputFormat === format ? (
                          <button
                            className="secondary"
                            onClick={() => call("open_output", { index })}
                          >
                            Show in Finder
                          </button>
                        ) : (
                          <button
                            className="icon-button"
                            disabled={selectionLocked}
                            onClick={() => removeImage(index)}
                            aria-label={`Remove ${item.image.name}`}
                          >
                            <HugeiconsIcon
                              icon={Cancel01Icon}
                              size={18}
                              aria-hidden="true"
                            />
                          </button>
                        )}
                      </li>
                    ))}
                  </ol>
                  <p className="field-help">
                    {multiple
                      ? "Games run one at a time. Completed output is kept if the queue stops."
                      : "Image checked means its files can be read; console compatibility is not verified."}
                  </p>
                </>
              )}
            </section>
            {s.status !== "completed" && (
              <section
                className="card destination-card"
                aria-labelledby="destination-heading"
              >
                <div className="section-label">
                  <span className="step" aria-hidden="true">
                    03
                  </span>
                  <h2 id="destination-heading">Destination</h2>
                </div>
                <div className="destination-controls">
                  <div className="location-control">
                    <span className="field-label">Save location</span>
                    <div className="folder-row">
                      <span className="folder-icon">
                        <Icon name="folder" size={26} />
                      </span>
                      <div className="folder-info">
                        <strong>
                          {s.destination?.split("/").pop() ||
                            "Choose a location"}
                        </strong>
                        {s.destination && (
                          <p className="path" title={s.destination}>
                            {s.destination}
                          </p>
                        )}
                      </div>
                      <button
                        className="secondary"
                        onClick={chooseFolder}
                        aria-label="Choose destination folder"
                        disabled={locked || !native}
                      >
                        <HugeiconsIcon
                          icon={FolderOpenIcon}
                          size={18}
                          aria-hidden="true"
                        />
                        {s.destination ? "Change folder" : "Choose folder"}
                      </button>
                    </div>
                  </div>
                  <div className="output-name">
                    {multiple ? (
                      <p className="hint">
                        Folder names come from each ISO. Existing folders are
                        preserved by adding a numbered suffix.
                      </p>
                    ) : (
                      <>
                        <label className="field-label" htmlFor="subfolder">
                          Output folder name
                        </label>
                        <input
                          id="subfolder"
                          value={name}
                          onChange={(e) => setName(e.target.value)}
                          disabled={locked || !s.image}
                          placeholder="Choose a game image first"
                          autoComplete="off"
                          spellCheck={false}
                        />
                        <p className="field-help">
                          A new folder inside your chosen location
                        </p>
                      </>
                    )}
                  </div>
                </div>
                {s.destination && s.image && (
                  <p className="output-preview">
                    <span>Output location</span>
                    <span
                      title={`${s.destination}/${multiple ? "<one folder per game>" : name}`}
                    >
                      {s.destination}/
                      {multiple
                        ? "<one folder per game>"
                        : name || "<folder name>"}
                    </span>
                  </p>
                )}
              </section>
            )}
          </>
        )}
        <section
          className={`card process-card ${s.status === "completed" ? "success-card" : ""}`}
          aria-label="Extraction status"
        >
          {s.status === "completed" && s.result ? (
            <>
              <div className="result-icon">
                <Icon name="check" size={36} />
              </div>
              <h2>
                {multiple
                  ? "Queue complete"
                  : format === "god"
                    ? "GOD package complete"
                    : "Extraction complete"}
              </h2>
              <p>
                {multiple ? `${completed} images · ` : ""}
                {s.result.files.toLocaleString()} files ·{" "}
                {bytes(s.result.bytes)}{" "}
                {format === "god" ? "packaged" : "extracted"}
                {multiple ? " in the last image" : ""}
              </p>
              <div className="result-path">{s.result.path}</div>
              <p className="xex-status">
                {format === "god"
                  ? "Copy the contents of this folder to your Xbox storage, keeping the package and its .data folder together. Activate your exploit before launching from My Games."
                  : s.result.hasXex
                    ? "default.xex found"
                    : "default.xex was not found. Original Xbox games may use default.xbe."}
              </p>
              <div className="actions centered">
                <button className="primary" onClick={() => call("open_output")}>
                  <Icon name="folder" size={18} />
                  Show in Finder
                </button>
                <button className="secondary" onClick={() => call("reset")}>
                  Extract another
                </button>
              </div>
            </>
          ) : busy ? (
            <>
              <div className="working-header">
                <div>
                  <h2>
                    {s.status === "extracting"
                      ? format === "god"
                        ? "Creating GOD…"
                        : "Extracting…"
                      : "Checking…"}
                    {multiple &&
                      ` (${Math.min(completed + 1, s.queue.length)} of ${s.queue.length})`}
                  </h2>
                </div>
                <span className="time">
                  {elapsed(s.progress.elapsedSeconds)}
                </span>
              </div>
              {s.status === "extracting" &&
              (s.progress.totalBytes || s.progress.totalFiles) ? (
                <>
                  <progress
                    value={percent}
                    max={100}
                    aria-label="Extraction progress"
                  />
                  <div className="progress-meta">
                    <span>
                      {s.progress.filesDone.toLocaleString()} /{" "}
                      {s.progress.totalFiles.toLocaleString()} files ·{" "}
                      {bytes(s.progress.bytesDone)} /{" "}
                      {bytes(s.progress.totalBytes)}
                    </span>
                    <strong>{percent}%</strong>
                  </div>
                  <p className="current-file" title={s.progress.currentFile}>
                    {s.progress.currentFile || "Preparing files…"}
                  </p>
                </>
              ) : (
                <progress aria-label="Checking image" />
              )}
              <button
                className="secondary cancel"
                disabled={cancelling}
                onClick={async () => {
                  setCancelling(true);
                  await call("cancel_extraction");
                }}
              >
                {cancelling
                  ? "Stopping safely…"
                  : multiple
                    ? "Stop Queue"
                    : "Cancel"}
              </button>
            </>
          ) : (
            <div className="ready-row">
              <div>
                {
                  <p className="ready-caption" role="status">
                    {s.partialPath
                      ? "Resolve partial files to continue"
                      : !s.image
                        ? "Add a game image to begin"
                        : !s.destination
                          ? "Choose where to save the output"
                          : !multiple && !name.trim()
                            ? "Give the output folder a name"
                            : `${s.queue.length} ${multiple ? "games" : "game"} · Ready to ${format === "god" ? "create GOD" : "extract"}`}
                  </p>
                }
              </div>
              <button
                className="primary"
                onClick={start}
                disabled={
                  !native ||
                  !s.image ||
                  !s.destination ||
                  (!multiple && !name.trim()) ||
                  !!s.partialPath
                }
              >
                {multiple
                  ? completed
                    ? "Continue Queue"
                    : format === "god"
                      ? "Convert Queue"
                      : "Extract Queue"
                  : format === "god"
                    ? "Create GOD"
                    : "Extract"}
                <HugeiconsIcon
                  icon={FileDownloadIcon}
                  size={18}
                  aria-hidden="true"
                />
              </button>
            </div>
          )}
        </section>
        {error && (
          <div
            className={`alert ${error.category === "cancelled" ? "neutral" : ""}`}
            role="alert"
          >
            <Icon
              name={error.category === "cancelled" ? "disc" : "warning"}
              size={21}
            />
            <div>
              <strong>
                {["cancelled", "interrupted"].includes(error.category)
                  ? error.message
                  : "Couldn't finish this image"}
              </strong>
              {!["cancelled", "interrupted"].includes(error.category) && (
                <p>{error.message}</p>
              )}
              <p>{error.recovery}</p>
              {error.category === "collision" && (
                <button
                  className="secondary"
                  onClick={chooseFolder}
                  disabled={locked}
                >
                  Choose Another Folder
                </button>
              )}
            </div>
          </div>
        )}
        {s.partialPath && !busy && (
          <section className="card partial-card">
            <h2>Partial files</h2>
            <p className="result-path">{s.partialPath}</p>
            <p>
              These files may be incomplete. Deletion applies only to this
              app-managed folder.
            </p>
            <div className="actions">
              <button className="secondary" onClick={() => call("open_output")}>
                Show in Finder
              </button>
              <button className="secondary" onClick={() => resolve(false)}>
                Keep Partial Files
              </button>
              <button className="danger" onClick={() => resolve(true)}>
                Delete Partial Files
              </button>
            </div>
          </section>
        )}
        {notice && (
          <p className="notice" role="status">
            {notice}
          </p>
        )}
        <dialog
          ref={detailsRef}
          className="details-sheet"
          aria-labelledby="sheet-title"
          onCancel={(event) => {
            event.preventDefault();
            closeDetails();
          }}
          onClick={(event) => {
            const bounds = event.currentTarget.getBoundingClientRect();
            if (event.clientX < bounds.left || event.clientX > bounds.right)
              closeDetails();
          }}
        >
          <header className="sheet-heading">
            <h2 id="sheet-title" tabIndex={-1}>
              Help &amp; details
            </h2>
            <button onClick={closeDetails} aria-label="Close details">
              <HugeiconsIcon icon={Cancel01Icon} size={20} aria-hidden="true" />
            </button>
          </header>
          <div
            className="sheet-guide"
            onDragStart={(event) => event.preventDefault()}
          >
            <h3>Two ways to play</h3>
            <p>
              <strong>Extracted files</strong> unpacks the game into a folder.
              Use Aurora or another launcher to play it.
            </p>
            <p>
              <strong>Games on Demand (GOD)</strong> creates a package for My
              Games on Microsoft's dashboard. The necessary console patches must
              be active. This experimental mode supports single-disc Xbox 360
              games; install and DLC discs need separate handling. Compatibility
              still needs testing on your console.
            </p>
            <h3>Where your game goes</h3>
            <p>
              Browse chooses where to save. Output folder name names the new
              folder created there. Your source ISO and existing folders are
              preserved.
            </p>
          </div>
          <details className="diagnostics-disclosure">
            <summary>Session diagnostics</summary>
            <div className="details-content">
              <p className="hint">Xbox / Xbox 360 · xdvdfs 0.8.3 · Offline</p>
              <p className="hint">
                Each disc gets its own folder. Existing folders are preserved.
              </p>
              <pre tabIndex={0}>
                {[...s.logs, error?.detail].filter(Boolean).join("\n")}
              </pre>
              <button className="secondary" onClick={copyDetails}>
                <Icon name="copy" size={15} />
                Copy diagnostics
              </button>
              <p className="hint">
                The copied report excludes full source and output paths. Review
                before sharing.
              </p>
            </div>
          </details>
        </dialog>
        <span className="sr-only" role="status" aria-live="polite">
          {s.status === "extracting"
            ? `Extracting, ${percent} percent`
            : s.status}
        </span>
      </main>
    </>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
