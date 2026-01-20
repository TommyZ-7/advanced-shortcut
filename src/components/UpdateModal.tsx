import { motion, AnimatePresence } from "framer-motion";
import {
  Download,
  RefreshCw,
  X,
  CheckCircle,
  AlertCircle,
  Loader2,
} from "lucide-react";
import { Button } from "./common";
import type {
  UpdateInfo,
  UpdateProgress,
  UpdateStatus,
} from "../hooks/useUpdater";

interface UpdateModalProps {
  isOpen: boolean;
  onClose: () => void;
  status: UpdateStatus;
  updateInfo: UpdateInfo | null;
  progress: UpdateProgress | null;
  error: string | null;
  onUpdate: () => void;
  onCheckAgain: () => void;
}

export function UpdateModal({
  isOpen,
  onClose,
  status,
  updateInfo,
  progress,
  error,
  onUpdate,
  onCheckAgain,
}: UpdateModalProps) {
  const canClose = status !== "downloading" && status !== "installing";

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.15 }}
          className="fixed inset-0 z-50 flex items-center justify-center p-4"
        >
          {/* Backdrop */}
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="absolute inset-0 bg-black/50 backdrop-blur-sm"
            onClick={canClose ? onClose : undefined}
          />

          {/* Modal Content */}
          <motion.div
            initial={{ opacity: 0, scale: 0.95, y: 10 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.95, y: 10 }}
            transition={{
              duration: 0.2,
              ease: [0.16, 1, 0.3, 1],
            }}
            className="relative max-w-md w-full bg-[#2d2d2d] rounded-lg shadow-2xl border border-white/10 overflow-hidden"
          >
            {/* Header */}
            <div className="flex items-center justify-between px-6 py-4 border-b border-white/10">
              <h2 className="text-lg font-semibold text-white flex items-center gap-2">
                <Download className="w-5 h-5 text-[#0078d4]" />
                アップデート
              </h2>
              {canClose && (
                <button
                  onClick={onClose}
                  className="p-1.5 rounded-md hover:bg-white/10 transition-colors"
                >
                  <X className="w-5 h-5 text-gray-400" />
                </button>
              )}
            </div>

            {/* Body */}
            <div className="p-6">
              {status === "checking" && <CheckingContent />}
              {status === "available" && updateInfo && (
                <AvailableContent
                  updateInfo={updateInfo}
                  onUpdate={onUpdate}
                  onClose={onClose}
                />
              )}
              {status === "downloading" && progress && (
                <DownloadingContent progress={progress} />
              )}
              {status === "installing" && <InstallingContent />}
              {status === "up-to-date" && (
                <UpToDateContent
                  onCheckAgain={onCheckAgain}
                  onClose={onClose}
                />
              )}
              {status === "error" && (
                <ErrorContent
                  error={error}
                  onCheckAgain={onCheckAgain}
                  onClose={onClose}
                />
              )}
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

// ========================================
// Content Components
// ========================================

function CheckingContent() {
  return (
    <div className="flex flex-col items-center py-8">
      <motion.div
        animate={{ rotate: 360 }}
        transition={{ duration: 1, repeat: Infinity, ease: "linear" }}
      >
        <RefreshCw className="w-12 h-12 text-[#0078d4]" />
      </motion.div>
      <p className="mt-4 text-white font-medium">アップデートを確認中...</p>
      <p className="mt-1 text-sm text-gray-400">しばらくお待ちください</p>
    </div>
  );
}

function AvailableContent({
  updateInfo,
  onUpdate,
  onClose,
}: {
  updateInfo: UpdateInfo;
  onUpdate: () => void;
  onClose: () => void;
}) {
  return (
    <div className="space-y-4">
      <div className="flex items-start gap-4">
        <div className="p-3 rounded-full bg-[#0078d4]/20 text-[#0078d4]">
          <Download className="w-6 h-6" />
        </div>
        <div className="flex-1">
          <h3 className="text-white font-medium">
            新しいバージョンが利用可能です
          </h3>
          <p className="text-sm text-gray-400 mt-1">
            v{updateInfo.currentVersion} → v{updateInfo.version}
          </p>
        </div>
      </div>

      {updateInfo.body && (
        <div className="bg-white/5 rounded-lg p-4 max-h-40 overflow-y-auto">
          <h4 className="text-sm font-medium text-gray-300 mb-2">更新内容</h4>
          <div className="text-sm text-gray-400 whitespace-pre-wrap">
            {updateInfo.body}
          </div>
        </div>
      )}

      <div className="flex justify-end gap-3 pt-2">
        <Button variant="ghost" onClick={onClose}>
          後で
        </Button>
        <Button variant="primary" onClick={onUpdate}>
          <Download className="w-4 h-4" />
          今すぐ更新
        </Button>
      </div>
    </div>
  );
}

function DownloadingContent({ progress }: { progress: UpdateProgress }) {
  const formatBytes = (bytes: number) => {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-4">
        <motion.div
          animate={{ rotate: 360 }}
          transition={{ duration: 1, repeat: Infinity, ease: "linear" }}
          className="text-[#0078d4]"
        >
          <Loader2 className="w-8 h-8" />
        </motion.div>
        <div className="flex-1">
          <h3 className="text-white font-medium">ダウンロード中...</h3>
          <p className="text-sm text-gray-400">
            {formatBytes(progress.downloaded)} / {formatBytes(progress.total)}
          </p>
        </div>
        <div className="text-lg font-bold text-[#0078d4]">
          {Math.round(progress.percent)}%
        </div>
      </div>

      {/* Progress Bar */}
      <div className="relative h-3 bg-white/10 rounded-full overflow-hidden">
        <motion.div
          className="absolute inset-y-0 left-0 bg-linear-to-r from-[#0078d4] to-[#1a86d9] rounded-full"
          initial={{ width: 0 }}
          animate={{ width: `${progress.percent}%` }}
          transition={{ duration: 0.3, ease: "easeOut" }}
        />
        {/* Shimmer effect */}
        <motion.div
          className="absolute inset-0 bg-linear-to-r from-transparent via-white/20 to-transparent"
          animate={{ x: ["-100%", "100%"] }}
          transition={{ duration: 1.5, repeat: Infinity, ease: "linear" }}
        />
      </div>

      <p className="text-center text-sm text-gray-500">
        ダウンロード完了後、自動的にインストールが開始されます
      </p>
    </div>
  );
}

function InstallingContent() {
  return (
    <div className="flex flex-col items-center py-8">
      <motion.div
        animate={{ scale: [1, 1.1, 1] }}
        transition={{ duration: 1.5, repeat: Infinity }}
        className="text-[#0078d4]"
      >
        <Loader2 className="w-12 h-12 animate-spin" />
      </motion.div>
      <p className="mt-4 text-white font-medium">インストール中...</p>
      <p className="mt-1 text-sm text-gray-400">
        完了後、アプリが再起動されます
      </p>
    </div>
  );
}

function UpToDateContent({
  onCheckAgain,
  onClose,
}: {
  onCheckAgain: () => void;
  onClose: () => void;
}) {
  return (
    <div className="space-y-4">
      <div className="flex flex-col items-center py-4">
        <div className="p-3 rounded-full bg-green-500/20 text-green-500">
          <CheckCircle className="w-10 h-10" />
        </div>
        <p className="mt-4 text-white font-medium">最新バージョンです</p>
        <p className="mt-1 text-sm text-gray-400">現在のバージョンは最新です</p>
      </div>

      <div className="flex justify-end gap-3">
        <Button variant="ghost" onClick={onCheckAgain}>
          <RefreshCw className="w-4 h-4" />
          再確認
        </Button>
        <Button variant="secondary" onClick={onClose}>
          閉じる
        </Button>
      </div>
    </div>
  );
}

function ErrorContent({
  error,
  onCheckAgain,
  onClose,
}: {
  error: string | null;
  onCheckAgain: () => void;
  onClose: () => void;
}) {
  return (
    <div className="space-y-4">
      <div className="flex items-start gap-4">
        <div className="p-3 rounded-full bg-red-500/20 text-red-500">
          <AlertCircle className="w-6 h-6" />
        </div>
        <div className="flex-1">
          <h3 className="text-white font-medium">エラーが発生しました</h3>
          <p className="text-sm text-gray-400 mt-1">
            {error || "アップデートの確認中にエラーが発生しました"}
          </p>
        </div>
      </div>

      <div className="flex justify-end gap-3">
        <Button variant="ghost" onClick={onCheckAgain}>
          <RefreshCw className="w-4 h-4" />
          再試行
        </Button>
        <Button variant="secondary" onClick={onClose}>
          閉じる
        </Button>
      </div>
    </div>
  );
}
