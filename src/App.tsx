import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import {
  Monitor,
  Fingerprint,
  RefreshCw,
  Globe,
  ChevronDown,
  X,
  Minus,
  CheckCircle2,
  AlertCircle,
  ShieldCheck,
  ShieldAlert,
  Crosshair,
  Sliders,
  Link2,
  Sparkles,
} from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface MonitorInfo {
  id: number;
  name: string;
  gdi_name: string;
  friendly_name: string;
  interface_path: string;
  hardware_id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  is_primary: boolean;
  hmonitor: number;
}

interface TouchDeviceInfo {
  id: number;
  product_name: string;
  device_path: string;
  reg_key_name: string;
  device_type: string;
  max_contacts: number;
  vendor_id: number;
  product_id: number;
  is_virtual: boolean;
  mapped_monitor_path: string | null;
  mapped_monitor_id: number | null;
  active_monitor_id: number | null;
}

interface SystemTouchState {
  monitors: MonitorInfo[];
  touch_devices: TouchDeviceInfo[];
  is_admin: boolean;
}

function App() {
  const { t, i18n } = useTranslation();
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [touchDevices, setTouchDevices] = useState<TouchDeviceInfo[]>([]);
  const [selectedMonitor, setSelectedMonitor] = useState<number>(0);
  const [selectedTouchId, setSelectedTouchId] = useState<number>(0);
  const [isAdmin, setIsAdmin] = useState<boolean>(false);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [isBinding, setIsBinding] = useState<boolean>(false);
  const [showVirtual, setShowVirtual] = useState<boolean>(false);
  const [showLangMenu, setShowLangMenu] = useState<boolean>(false);
  const [toast, setToast] = useState<{
    text: string;
    type: "success" | "error";
  } | null>(null);

  const appWindow = getCurrentWindow();

  const showToast = useCallback(
    (text: string, type: "success" | "error" = "success") => {
      setToast({ text, type });
      setTimeout(() => {
        setToast((prev) => (prev?.text === text ? null : prev));
      }, 4000);
    },
    []
  );

  const fetchSystemState = useCallback(
    async (preserveSelection = true) => {
      setIsLoading(true);
      try {
        const state = await invoke<SystemTouchState>("get_touch_system_state");
        setMonitors(state.monitors);
        setTouchDevices(state.touch_devices);
        setIsAdmin(state.is_admin);

        if (!preserveSelection) {
          // Restore saved monitor preference or default to currently mapped / primary monitor
          const savedMonName = localStorage.getItem("selected_monitor_name");
          const savedMonIdx = savedMonName
            ? state.monitors.findIndex(
                (m) => m.gdi_name === savedMonName || m.name === savedMonName
              )
            : -1;

          const physicalTouch = state.touch_devices.find((d) => !d.is_virtual);
          if (physicalTouch) {
            setSelectedTouchId(physicalTouch.id);
            if (savedMonIdx >= 0) {
              setSelectedMonitor(savedMonIdx);
            } else if (physicalTouch.active_monitor_id !== null) {
              setSelectedMonitor(physicalTouch.active_monitor_id);
            } else {
              const nonPrimary = state.monitors.findIndex((m) => !m.is_primary);
              setSelectedMonitor(nonPrimary >= 0 ? nonPrimary : 0);
            }
          } else if (savedMonIdx >= 0) {
            setSelectedMonitor(savedMonIdx);
          }
        }
      } catch (err) {
        console.error("Failed to fetch touch system state:", err);
      } finally {
        setIsLoading(false);
      }
    },
    []
  );

  useEffect(() => {
    fetchSystemState(false);
  }, [fetchSystemState]);

  useEffect(() => {
    invoke("update_tray_locale", {
      showText: t("trayShow"),
      exitText: t("trayExit"),
    }).catch(console.error);
  }, [i18n.language, t]);

  const visibleTouchDevices = touchDevices.filter(
    (d) => showVirtual || !d.is_virtual
  );

  const activeTouchDevice =
    visibleTouchDevices.find((d) => d.id === selectedTouchId) ??
    visibleTouchDevices[0] ??
    null;

  const targetMonitor =
    monitors.find((m) => m.id === selectedMonitor) ?? monitors[0] ?? null;

  const handleBindTouch = async () => {
    if (!activeTouchDevice || !targetMonitor) return;
    setIsBinding(true);
    try {
      await invoke<string>("bind_touch_to_monitor", {
        devicePath: activeTouchDevice.device_path,
        monitorInterfacePath: targetMonitor.interface_path,
      });
      await fetchSystemState(true);
      showToast(
        t("bindSuccess", {
          monitor: `${targetMonitor.id + 1} (${targetMonitor.friendly_name})`,
        }),
        "success"
      );
    } catch (err: any) {
      showToast(t("bindError", { error: String(err) }), "error");
    } finally {
      setIsBinding(false);
    }
  };

  const handleOpenNativeTool = async (mode: "multidigimon" | "tabcal") => {
    try {
      await invoke("open_native_touch_tool", { mode });
    } catch (err: any) {
      showToast(String(err), "error");
    }
  };

  const languages = [
    { code: "es", label: "Español" },
    { code: "en", label: "English" },
    { code: "fr", label: "Français" },
    { code: "de", label: "Deutsch" },
    { code: "ja", label: "日本語" },
  ];

  const currentLang =
    languages.find((l) => i18n.resolvedLanguage?.startsWith(l.code)) ??
    languages[0];

  const isAlreadyBoundHere =
    activeTouchDevice &&
    targetMonitor &&
    (activeTouchDevice.mapped_monitor_id === targetMonitor.id ||
      activeTouchDevice.active_monitor_id === targetMonitor.id);

  return (
    <div className="flex flex-col h-screen overflow-hidden rounded-xl border border-border bg-surface-0 shadow-2xl">
      {/* Custom Frameless Title Bar (from mousejiggler) */}
      <div
        data-tauri-drag-region
        className="h-9 shrink-0 flex items-center justify-between px-3.5 bg-surface-1 border-b border-border select-none"
      >
        <span className="text-[12px] text-text-primary font-semibold tracking-wide pointer-events-none flex items-center gap-2">
          <Fingerprint size={15} className="text-accent" />
          {t("appTitle")}
        </span>
        <div className="flex items-center gap-1">
          <button
            onClick={() => appWindow.minimize()}
            className="p-1.5 rounded hover:bg-surface-3 text-text-secondary hover:text-text-primary transition-colors cursor-pointer"
            title="Minimize"
          >
            <Minus size={14} />
          </button>
          <button
            onClick={() => appWindow.hide()}
            className="p-1.5 rounded hover:bg-red-500/90 text-text-secondary hover:text-white transition-colors cursor-pointer"
            title="Close to tray"
          >
            <X size={14} />
          </button>
        </div>
      </div>

      {/* Main Content */}
      <div className="flex-1 flex flex-col px-4 pb-3 pt-3 gap-3 overflow-y-auto">
        {/* Top bar: subtitle + language + refresh */}
        <div className="flex items-center justify-between gap-2">
          <p className="text-xs text-text-secondary">{t("appSubtitle")}</p>

          <div className="flex items-center gap-1.5 shrink-0">
            <button
              onClick={() => fetchSystemState(true)}
              disabled={isLoading}
              className="flex items-center gap-1 text-[11px] text-text-secondary hover:text-text-primary px-2 py-1 rounded-md bg-surface-1 hover:bg-surface-2 border border-border transition-colors cursor-pointer"
              title={t("refreshBtn")}
            >
              <RefreshCw size={12} className={isLoading ? "spin" : ""} />
              <span>{t("refreshBtn")}</span>
            </button>

            {/* Language Selector */}
            <div className="relative">
              <button
                onClick={() => setShowLangMenu(!showLangMenu)}
                className="flex items-center gap-1.5 text-[11px] text-text-secondary hover:text-text-primary px-2 py-1 rounded-md bg-surface-1 hover:bg-surface-2 border border-border transition-colors cursor-pointer"
              >
                <Globe size={12} />
                <span>{currentLang.label}</span>
                <ChevronDown
                  size={11}
                  className={`transition-transform ${
                    showLangMenu ? "rotate-180" : ""
                  }`}
                />
              </button>
              {showLangMenu && (
                <>
                  <div
                    className="fixed inset-0 z-20"
                    onClick={() => setShowLangMenu(false)}
                  />
                  <div className="absolute right-0 top-full mt-1 z-30 bg-surface-2 border border-border rounded-lg shadow-xl overflow-hidden min-w-[130px]">
                    {languages.map((lang) => (
                      <button
                        key={lang.code}
                        onClick={() => {
                          i18n.changeLanguage(lang.code);
                          setShowLangMenu(false);
                        }}
                        className={`w-full text-left text-[12px] px-3 py-1.5 transition-colors cursor-pointer ${
                          currentLang.code === lang.code
                            ? "bg-accent text-surface-0 font-semibold"
                            : "text-text-secondary hover:text-text-primary hover:bg-surface-3"
                        }`}
                      >
                        {lang.label}
                      </button>
                    ))}
                  </div>
                </>
              )}
            </div>
          </div>
        </div>

        {/* SECTION 1: Monitor Selector (Proportional Map from kindleCustomSO + Detail Pills) */}
        <div className="flex flex-col gap-1.5 p-3 rounded-xl bg-surface-1 border border-border">
          <div className="flex items-center justify-between">
            <label className="text-[12px] font-semibold text-text-primary flex items-center gap-1.5">
              <Monitor size={14} className="text-accent" />
              {t("monitorsSection")}
            </label>
            {targetMonitor && (
              <span className="text-[11px] text-accent font-medium truncate max-w-[230px]">
                #{targetMonitor.id + 1} · {targetMonitor.friendly_name} (
                {targetMonitor.width}×{targetMonitor.height})
              </span>
            )}
          </div>

          {monitors.length > 0 &&
            (() => {
              const minX = Math.min(...monitors.map((m) => m.x));
              const minY = Math.min(...monitors.map((m) => m.y));
              const totalW =
                Math.max(...monitors.map((m) => m.x + m.width)) - minX || 1920;
              const totalH =
                Math.max(...monitors.map((m) => m.y + m.height)) - minY || 1080;

              return (
                <div
                  className="monitor-layout"
                  style={{ aspectRatio: `${totalW} / ${totalH}` }}
                >
                  {monitors.map((m) => {
                    const isTouchHere =
                      activeTouchDevice?.mapped_monitor_id === m.id ||
                      activeTouchDevice?.active_monitor_id === m.id;
                    return (
                      <button
                        key={m.id}
                        type="button"
                        className={`monitor-tile ${
                          selectedMonitor === m.id ? "selected" : ""
                        } ${isTouchHere ? "mapped-active" : ""}`}
                        style={{
                          left: `${((m.x - minX) / totalW) * 100}%`,
                          top: `${((m.y - minY) / totalH) * 100}%`,
                          width: `${(m.width / totalW) * 100}%`,
                          height: `${(m.height / totalH) * 100}%`,
                        }}
                        onClick={() => {
                          setSelectedMonitor(m.id);
                          try {
                            localStorage.setItem(
                              "selected_monitor_name",
                              m.gdi_name
                            );
                          } catch {}
                        }}
                        title={`${m.name} — ${m.width}×${m.height}`}
                      >
                        <div className="flex items-center gap-1">
                          <strong>
                            {m.id + 1}
                            {m.is_primary ? " ★" : ""}
                          </strong>
                          {isTouchHere && (
                            <Fingerprint
                              size={12}
                              className="text-emerald-400 shrink-0"
                            />
                          )}
                        </div>
                        <span className="text-[10px] font-medium truncate max-w-[92%] opacity-90">
                          {m.friendly_name}
                        </span>
                        <span className="text-[10px] opacity-75">
                          {m.width}×{m.height}
                        </span>
                      </button>
                    );
                  })}
                </div>
              );
            })()}

          {/* Quick Monitor Cards Row */}
          <div className="grid grid-cols-2 gap-2 mt-1">
            {monitors.map((m) => {
              const isSelected = selectedMonitor === m.id;
              const isTouchHere =
                activeTouchDevice?.mapped_monitor_id === m.id ||
                activeTouchDevice?.active_monitor_id === m.id;

              return (
                <button
                  key={m.id}
                  type="button"
                  onClick={() => {
                    setSelectedMonitor(m.id);
                    try {
                      localStorage.setItem("selected_monitor_name", m.gdi_name);
                    } catch {}
                  }}
                  className={`flex items-center justify-between px-2.5 py-2 rounded-lg border text-left transition-all cursor-pointer ${
                    isSelected
                      ? "bg-accent/12 border-accent text-text-primary"
                      : "bg-surface-0/70 border-border hover:border-border-hover text-text-secondary"
                  }`}
                >
                  <div className="min-w-0">
                    <div className="flex items-center gap-1.5">
                      <span className="text-xs font-bold text-text-primary">
                        #{m.id + 1} {m.friendly_name}
                      </span>
                      {m.is_primary && (
                        <span className="text-[9px] px-1.5 py-0.2 rounded bg-surface-3 text-text-secondary font-medium">
                          ★
                        </span>
                      )}
                    </div>
                    <div className="text-[10px] text-text-secondary truncate">
                      {m.gdi_name.replace("\\\\.\\", "")} · {m.width}×{m.height}
                    </div>
                  </div>
                  {isTouchHere && (
                    <span className="shrink-0 flex items-center gap-1 text-[10px] font-semibold px-1.5 py-0.5 rounded-full bg-emerald-500/15 text-emerald-400 border border-emerald-500/30">
                      <Fingerprint size={11} />
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        </div>

        {/* SECTION 2: Touch Digitizers (HID) */}
        <div className="flex flex-col gap-2 p-3 rounded-xl bg-surface-1 border border-border">
          <div className="flex items-center justify-between">
            <label className="text-[12px] font-semibold text-text-primary flex items-center gap-1.5">
              <Fingerprint size={14} className="text-accent" />
              {t("digitizersSection")}
            </label>
            {touchDevices.some((d) => d.is_virtual) && (
              <label className="flex items-center gap-1.5 text-[11px] text-text-secondary cursor-pointer hover:text-text-primary">
                <input
                  type="checkbox"
                  checked={showVirtual}
                  onChange={(e) => setShowVirtual(e.target.checked)}
                  className="accent-amber-500 rounded cursor-pointer"
                />
                <span>{t("showVirtual")}</span>
              </label>
            )}
          </div>

          {visibleTouchDevices.length === 0 ? (
            <div className="py-4 text-center text-xs text-text-secondary bg-surface-0/50 rounded-lg border border-dashed border-border">
              {t("noTouchDevices")}
            </div>
          ) : (
            <div className="flex flex-col gap-1.5 max-h-[120px] overflow-y-auto pr-0.5">
              {visibleTouchDevices.map((dev) => {
                const isSelected = activeTouchDevice?.id === dev.id;
                const mappedMon = monitors.find(
                  (m) =>
                    m.id === (dev.mapped_monitor_id ?? dev.active_monitor_id)
                );

                return (
                  <div
                    key={dev.id}
                    onClick={() => setSelectedTouchId(dev.id)}
                    className={`flex items-center justify-between p-2.5 rounded-lg border transition-all cursor-pointer ${
                      isSelected
                        ? "bg-accent/10 border-accent"
                        : "bg-surface-0/60 border-border hover:border-border-hover"
                    }`}
                  >
                    <div className="flex items-center gap-2.5 min-w-0">
                      <div
                        className={`w-7 h-7 rounded-lg flex items-center justify-center shrink-0 ${
                          isSelected
                            ? "bg-accent text-surface-0"
                            : "bg-surface-2 text-text-secondary"
                        }`}
                      >
                        <Fingerprint size={16} />
                      </div>
                      <div className="min-w-0">
                        <div className="flex items-center gap-1.5">
                          <span className="text-xs font-semibold text-text-primary truncate">
                            {dev.product_name}
                          </span>
                          {dev.vendor_id > 0 && (
                            <span className="text-[10px] font-mono px-1.5 py-0.2 rounded bg-surface-2 text-text-secondary">
                              {dev.vendor_id
                                .toString(16)
                                .toUpperCase()
                                .padStart(4, "0")}
                              :
                              {dev.product_id
                                .toString(16)
                                .toUpperCase()
                                .padStart(4, "0")}
                            </span>
                          )}
                          <span className="text-[10px] px-1.5 py-0.2 rounded bg-surface-2 text-text-secondary">
                            {dev.max_contacts} {t("contactsLabel")}
                          </span>
                        </div>
                        <div className="text-[10px] text-text-secondary truncate mt-0.5">
                          {t("currentlyMappedTo")}{" "}
                          {mappedMon ? (
                            <strong className="text-emerald-400">
                              #{mappedMon.id + 1} {mappedMon.friendly_name} (
                              {mappedMon.width}×{mappedMon.height})
                            </strong>
                          ) : (
                            <span className="text-amber-400">
                              {t("unmapped")}
                            </span>
                          )}
                        </div>
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          )}

          {/* Primary Action Button (Option B: Direct Registry + WM_SETTINGCHANGE) */}
          <button
            type="button"
            onClick={handleBindTouch}
            disabled={isBinding || !activeTouchDevice || !targetMonitor}
            className={`mt-1 w-full py-2.5 px-4 rounded-lg font-semibold text-xs flex items-center justify-center gap-2 transition-all cursor-pointer shadow-lg ${
              isAlreadyBoundHere
                ? "bg-emerald-500 hover:bg-emerald-400 text-surface-0 shadow-emerald-500/20"
                : "bg-accent hover:bg-accent-hover text-surface-0 shadow-amber-500/20"
            } disabled:opacity-50 disabled:cursor-not-allowed`}
          >
            {isBinding ? (
              <>
                <RefreshCw size={15} className="spin" />
                <span>{t("bindingInProgress")}</span>
              </>
            ) : isAlreadyBoundHere ? (
              <>
                <Sparkles size={15} />
                <span>{t("alreadyBoundButton")}</span>
              </>
            ) : (
              <>
                <Link2 size={15} />
                <span>{t("bindButton")}</span>
              </>
            )}
          </button>
        </div>

        {/* SECTION 3: Option A (Native Windows Tools: MultiDigiMon & tabcal) */}
        <div className="flex flex-col gap-1.5 p-2.5 rounded-xl bg-surface-1/70 border border-border">
          <span className="text-[11px] font-medium text-text-secondary flex items-center gap-1.5">
            <Sliders size={12} />
            {t("nativeToolsTitle")}
          </span>
          <div className="grid grid-cols-2 gap-2">
            <button
              type="button"
              onClick={() => handleOpenNativeTool("multidigimon")}
              className="flex items-center gap-2 p-2 rounded-lg bg-surface-0/80 hover:bg-surface-2 border border-border hover:border-border-hover text-left transition-colors cursor-pointer"
            >
              <Monitor size={15} className="text-accent shrink-0" />
              <div className="min-w-0">
                <div className="text-[11px] font-semibold text-text-primary truncate">
                  {t("openMultiDigiMon")}
                </div>
                <div className="text-[9px] font-mono text-text-secondary truncate">
                  {t("openMultiDigiMonDesc")}
                </div>
              </div>
            </button>

            <button
              type="button"
              onClick={() => handleOpenNativeTool("tabcal")}
              className="flex items-center gap-2 p-2 rounded-lg bg-surface-0/80 hover:bg-surface-2 border border-border hover:border-border-hover text-left transition-colors cursor-pointer"
            >
              <Crosshair size={15} className="text-emerald-400 shrink-0" />
              <div className="min-w-0">
                <div className="text-[11px] font-semibold text-text-primary truncate">
                  {t("openTabCal")}
                </div>
                <div className="text-[9px] font-mono text-text-secondary truncate">
                  {t("openTabCalDesc")}
                </div>
              </div>
            </button>
          </div>
        </div>

        {/* Toast Feedback */}
        {toast && (
          <div
            className={`flex items-center gap-2 px-3 py-2 rounded-lg text-xs font-medium border ${
              toast.type === "success"
                ? "bg-emerald-950/80 border-emerald-500/50 text-emerald-300"
                : "bg-rose-950/80 border-rose-500/50 text-rose-300"
            }`}
          >
            {toast.type === "success" ? (
              <CheckCircle2 size={15} className="shrink-0" />
            ) : (
              <AlertCircle size={15} className="shrink-0" />
            )}
            <span className="truncate">{toast.text}</span>
          </div>
        )}

        {/* Footer Status Bar */}
        <div className="mt-auto flex items-center justify-center gap-1.5 pt-1 text-[10px] text-text-secondary">
          {isAdmin ? (
            <>
              <ShieldCheck size={12} className="text-emerald-400" />
              <span>{t("adminMode")}</span>
            </>
          ) : (
            <>
              <ShieldAlert size={12} className="text-amber-400" />
              <span>{t("uacMode")}</span>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

export default App;
