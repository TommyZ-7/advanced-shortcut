import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { Sidebar, TitleBar, PageContainer } from "./components/layout";
import {
  ShortcutsPage,
  GroupsPage,
  SettingsPage,
  AboutPage,
} from "./components/pages";
import { ShortcutEditor } from "./components/ShortcutEditor";
import { ConfirmDialog, Spinner } from "./components/common";
import { useAppData } from "./hooks/useAppData";
import type { Shortcut, Group, CliShortcutRequest } from "./types";

function App() {
  const [currentPage, setCurrentPage] = useState("shortcuts");
  const [editorOpen, setEditorOpen] = useState(false);
  const [editingShortcut, setEditingShortcut] = useState<Shortcut | null>(null);
  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false);
  const [shortcutToDelete, setShortcutToDelete] = useState<Shortcut | null>(
    null,
  );
  const [cliRequest, setCliRequest] = useState<CliShortcutRequest | null>(null);
  const [cliStatus, setCliStatus] = useState<
    "idle" | "running" | "success" | "error"
  >("idle");
  const [cliError, setCliError] = useState<string | null>(null);

  const {
    shortcuts,
    groups,
    loading,
    loadData,
    addShortcut,
    updateShortcut,
    updateShortcuts,
    deleteShortcut,
    addGroup,
    updateGroup,
    deleteGroup,
    executeShortcut,
    saveGroups,
  } = useAppData();

  useEffect(() => {
    const loadCliRequest = async () => {
      try {
        const request = await invoke<CliShortcutRequest | null>(
          "get_cli_shortcut_request",
        );
        if (request) {
          setCliRequest(request);
          setCliStatus("idle");
          setCliError(null);
        }
      } catch (err) {
        console.error("Failed to read CLI request:", err);
      }
    };

    loadCliRequest();
  }, []);

  useEffect(() => {
    if (!cliRequest || loading || cliStatus !== "idle") return;

    if (shortcuts.length === 0) {
      loadData();
      return;
    }

    const shortcut = shortcuts.find((s) => s.id === cliRequest.shortcutId);
    if (!shortcut) {
      setCliError("ショートカットが見つかりませんでした");
      setCliStatus("error");
      if (cliRequest.closeAfterExecution) {
        invoke("exit_app", { code: 1 });
      } else {
        setCurrentPage("shortcuts");
        setCliRequest(null);
      }
      return;
    }

    let cancelled = false;
    const run = async () => {
      setCliStatus("running");
      try {
        await executeShortcut(shortcut);
        if (cancelled) return;
        setCliStatus("success");
        if (cliRequest.closeAfterExecution) {
          await invoke("exit_app", { code: 0 });
          return;
        }
        setCurrentPage("shortcuts");
        setCliRequest(null);
      } catch (err) {
        if (cancelled) return;
        setCliError(err instanceof Error ? err.message : String(err));
        setCliStatus("error");
        if (cliRequest.closeAfterExecution) {
          await invoke("exit_app", { code: 1 });
          return;
        }
        setCurrentPage("shortcuts");
        setCliRequest(null);
      }
    };

    run();
    return () => {
      cancelled = true;
    };
  }, [cliRequest, cliStatus, loading, shortcuts, executeShortcut, loadData]);

  const handleCreateShortcut = () => {
    setEditingShortcut(null);
    setEditorOpen(true);
  };

  const handleEditShortcut = (shortcut: Shortcut) => {
    setEditingShortcut(shortcut);
    setEditorOpen(true);
  };

  const handleSaveShortcut = async (shortcut: Shortcut) => {
    if (editingShortcut) {
      await updateShortcut(shortcut);
    } else {
      await addShortcut(shortcut);
    }
  };

  const handleDeleteShortcut = (shortcut: Shortcut) => {
    setShortcutToDelete(shortcut);
    setDeleteConfirmOpen(true);
  };

  const handleConfirmDelete = async () => {
    if (shortcutToDelete) {
      await deleteShortcut(shortcutToDelete.id);
      setShortcutToDelete(null);
    }
  };

  const handleToggleGroup = useCallback(
    async (groupId: string) => {
      const group = groups.find((g) => g.id === groupId);
      if (group) {
        await updateGroup({ ...group, isExpanded: !group.isExpanded });
      }
    },
    [groups, updateGroup],
  );

  const handleReorderShortcuts = useCallback(
    async (_groupId: string, newOrder: Shortcut[]) => {
      const reorderedShortcuts = newOrder.map((s, index) => ({
        ...s,
        order: index,
      }));
      await updateShortcuts(reorderedShortcuts);
    },
    [updateShortcuts],
  );

  const handleReorderGroups = useCallback(
    async (newGroups: Group[]) => {
      await saveGroups(newGroups);
    },
    [saveGroups],
  );

  const renderPage = () => {
    switch (currentPage) {
      case "shortcuts":
        return (
          <ShortcutsPage
            shortcuts={shortcuts}
            groups={groups}
            loading={loading}
            onExecute={executeShortcut}
            onEdit={handleEditShortcut}
            onDelete={handleDeleteShortcut}
            onCreate={handleCreateShortcut}
            onToggleGroup={handleToggleGroup}
            onReorderShortcuts={handleReorderShortcuts}
          />
        );
      case "groups":
        return (
          <GroupsPage
            groups={groups}
            onUpdate={updateGroup}
            onCreate={addGroup}
            onDelete={deleteGroup}
            onReorder={handleReorderGroups}
          />
        );
      case "settings":
        return <SettingsPage />;
      case "about":
        return <AboutPage />;
      default:
        return null;
    }
  };

  const showProgressScreen =
    cliRequest?.showProgressWindow &&
    (cliStatus === "running" || cliStatus === "error");

  return (
    <div className="flex flex-col h-full bg-[#1a1a1a]">
      <TitleBar />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar currentPage={currentPage} onPageChange={setCurrentPage} />
        <PageContainer currentPage={currentPage}>
          {showProgressScreen ? (
            <div className="h-full w-full flex items-center justify-center p-8">
              <div className="w-full max-w-lg bg-[#2d2d2d] border border-white/10 rounded-xl shadow-2xl p-8 text-center space-y-4">
                <div className="flex items-center justify-center">
                  <Spinner size="lg" />
                </div>
                <h2 className="text-lg font-semibold text-white">
                  ショートカットを実行中...
                </h2>
                <p className="text-sm text-gray-400">
                  処理が完了するまでお待ちください
                </p>
                {cliStatus === "error" && cliError && (
                  <p className="text-sm text-red-400">{cliError}</p>
                )}
              </div>
            </div>
          ) : (
            renderPage()
          )}
        </PageContainer>
      </div>

      <ShortcutEditor
        isOpen={editorOpen}
        onClose={() => setEditorOpen(false)}
        shortcut={editingShortcut}
        groups={groups}
        onSave={handleSaveShortcut}
      />

      <ConfirmDialog
        isOpen={deleteConfirmOpen}
        onClose={() => {
          setDeleteConfirmOpen(false);
          setShortcutToDelete(null);
        }}
        onConfirm={handleConfirmDelete}
        title="ショートカットを削除"
        message={`「${shortcutToDelete?.name || ""}」を削除しますか？この操作は取り消せません。`}
        confirmText="削除"
        cancelText="キャンセル"
        variant="danger"
      />
    </div>
  );
}

export default App;
