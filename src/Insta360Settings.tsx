import { useEffect, useRef, useState } from "react";
import { Check, CircleAlert, Download, ExternalLink, FolderOpen, LoaderCircle, RefreshCw, Search } from "lucide-react";
import { detectInsta360Sdk, onInsta360SdkProgress, openInsta360SdkDownload, pickFolder, setupInsta360Sdk } from "./tauri";
import type { Insta360SdkStatus } from "./types";

export function Insta360Settings({ path, onChange, onBusyChange }: { path: string; onChange: (path: string) => void; onBusyChange: (busy: boolean) => void }) {
  const [status, setStatus] = useState<Insta360SdkStatus>();
  const [busy, setBusy] = useState(false);
  const [checking, setChecking] = useState(true);
  const [progress, setProgress] = useState("");
  const [error, setError] = useState("");
  const [downloadUrl, setDownloadUrl] = useState("");
  const busyRef = useRef(false);
  const request = useRef(0);

  useEffect(() => {
    let disposed = false;
    const subscription = onInsta360SdkProgress((message) => { if (busyRef.current) setProgress(message); });
    subscription.then((unlisten) => { if (disposed) unlisten(); }).catch(() => {});
    return () => { disposed = true; subscription.then((unlisten) => unlisten()).catch(() => {}); };
  }, []);

  useEffect(() => {
    if (busyRef.current) return;
    const current = ++request.current;
    setChecking(true);
    const timer = window.setTimeout(() => {
      detectInsta360Sdk(path).then((value) => { if (current === request.current) { setStatus(value); setError(""); } })
        .catch((failure: unknown) => { if (current === request.current) setError(String(failure)); })
        .finally(() => { if (current === request.current) setChecking(false); });
    }, 250);
    return () => { window.clearTimeout(timer); if (current === request.current) request.current++; };
  }, [path]);

  const runSetup = async (url?: string) => {
    if (busyRef.current) return;
    request.current++;
    busyRef.current = true;
    onBusyChange(true);
    setBusy(true); setChecking(false); setError("");
    setProgress(url ? "正在准备官方下载…" : "正在搜索本机 SDK 与官方下载…");
    try {
      const value = await setupInsta360Sdk(path, url);
      setStatus(value);
      if (value.path) { onChange(value.path); setDownloadUrl(""); }
    } catch (failure) { setError(String(failure)); }
    finally { busyRef.current = false; onBusyChange(false); setBusy(false); setProgress(""); }
  };

  const recheck = async () => {
    const current = ++request.current;
    setChecking(true); setError("");
    try { const value = await detectInsta360Sdk(path); if (current === request.current) setStatus(value); }
    catch (failure) { if (current === request.current) setError(String(failure)); }
    finally { if (current === request.current) setChecking(false); }
  };
  const openOfficial = async () => {
    try { await openInsta360SdkDownload(); } catch (failure) { setError(String(failure)); }
  };
  const unsupported = status?.state === "unsupported";

  return <div className="modal-section sdk-settings" aria-busy={busy || checking}>
    <h3>Insta360 全景处理</h3>
    <div className={`sdk-state ${status?.state === "ready" ? "ready" : ""}`} role="status">
      {busy || checking ? <LoaderCircle size={15} className="spin" /> : status?.state === "ready" ? <Check size={15} /> : <CircleAlert size={15} />}
      <span>{busy ? progress : checking ? "正在检测 Desktop Media SDK…" : status?.message}</span>
    </div>
    <label className="setting-field"><span>Desktop Media SDK 目录</span><div className="sdk-path-row"><input aria-label="Desktop Media SDK 目录" value={path} disabled={busy} placeholder="留空时自动检测程序旁的 insta360/" onChange={(event) => onChange(event.target.value)} /><button className="secondary-button" aria-label="选择 SDK 目录" title="选择 SDK 目录" disabled={busy} onClick={async () => { try { const paths = await pickFolder(); if (paths[0]) onChange(paths[0]); } catch (failure) { setError(String(failure)); } }}><FolderOpen size={15} /></button><button className="secondary-button" aria-label="重新检测 SDK" title="重新检测 SDK" disabled={busy || checking} onClick={recheck}><RefreshCw size={14} /></button></div></label>
    <div className="sdk-actions"><button className="secondary-button" disabled={busy || unsupported} onClick={() => runSetup()}>{busy ? <LoaderCircle size={14} className="spin" /> : <Search size={14} />}{busy ? "查找与配置中…" : "自动查找并下载"}</button><button className="secondary-button" onClick={openOfficial}><ExternalLink size={14} />官方申请 / 下载</button></div>
    <small className="field-note">先查找本机 SDK 和下载目录中的 ZIP，再检查官方下载。找到后自动填入目录，保存设置即可使用。</small>
    {!unsupported && <details className="sdk-direct"><summary>使用官方 ZIP 下载直链</summary><div className="sdk-url-row"><input aria-label="官方 SDK ZIP 下载直链" type="url" value={downloadUrl} disabled={busy} placeholder="https://…insta360.com/…zip" onChange={(event) => setDownloadUrl(event.target.value)} /><button className="secondary-button" disabled={busy || !downloadUrl.trim()} onClick={() => runSetup(downloadUrl)}><Download size={14} />下载并配置</button></div><small className="field-note">官方包可能需登录申请。可填入账号取得的官方 HTTPS 直链，程序会自动下载并解压到 insta360/。</small></details>}
    {error && <div className="sdk-error" role="alert">{error}</div>}
  </div>;
}
