import { SectionHeader, Card, Toggle } from "../common";
import { Monitor, Moon, Bell } from "lucide-react";
import { useState } from "react";

export function SettingsPage() {
  const [settings, setSettings] = useState({
    darkMode: true,
    notifications: true,
    startMinimized: false,
    runAtStartup: false,
  });

  return (
    <div className="space-y-6">
      <SectionHeader
        title="設定"
        description="アプリケーションの動作をカスタマイズ"
      />

      <Card hover={false} className="divide-y divide-white/5">
        <SettingItem
          icon={<Moon className="w-5 h-5" />}
          title="ダークモード"
          description="常にダークテーマを使用します"
        >
          <Toggle
            checked={settings.darkMode}
            onChange={(checked) =>
              setSettings({ ...settings, darkMode: checked })
            }
          />
        </SettingItem>

        <SettingItem
          icon={<Bell className="w-5 h-5" />}
          title="通知"
          description="ショートカット実行時に通知を表示"
        >
          <Toggle
            checked={settings.notifications}
            onChange={(checked) =>
              setSettings({ ...settings, notifications: checked })
            }
          />
        </SettingItem>

        <SettingItem
          icon={<Monitor className="w-5 h-5" />}
          title="最小化して起動"
          description="アプリ起動時にシステムトレイに最小化"
        >
          <Toggle
            checked={settings.startMinimized}
            onChange={(checked) =>
              setSettings({ ...settings, startMinimized: checked })
            }
          />
        </SettingItem>

        <SettingItem
          icon={<Monitor className="w-5 h-5" />}
          title="スタートアップ時に起動"
          description="Windows起動時に自動的に起動"
        >
          <Toggle
            checked={settings.runAtStartup}
            onChange={(checked) =>
              setSettings({ ...settings, runAtStartup: checked })
            }
          />
        </SettingItem>
      </Card>

      <Card hover={false} className="p-6">
        <h3 className="text-lg font-semibold text-white mb-4">データ管理</h3>
        <div className="space-y-3">
          <button className="w-full text-left px-4 py-3 rounded-lg bg-white/5 hover:bg-white/10 transition-colors">
            <span className="text-white">データをエクスポート</span>
            <p className="text-sm text-gray-500">
              ショートカットとグループをJSONファイルに保存
            </p>
          </button>
          <button className="w-full text-left px-4 py-3 rounded-lg bg-white/5 hover:bg-white/10 transition-colors">
            <span className="text-white">データをインポート</span>
            <p className="text-sm text-gray-500">
              JSONファイルからデータを読み込み
            </p>
          </button>
          <button className="w-full text-left px-4 py-3 rounded-lg bg-red-500/10 hover:bg-red-500/20 transition-colors">
            <span className="text-red-400">すべてのデータを削除</span>
            <p className="text-sm text-gray-500">
              ショートカットとグループをすべて削除
            </p>
          </button>
        </div>
      </Card>
    </div>
  );
}

interface SettingItemProps {
  icon: React.ReactNode;
  title: string;
  description: string;
  children: React.ReactNode;
}

function SettingItem({ icon, title, description, children }: SettingItemProps) {
  return (
    <div className="flex items-center justify-between p-4">
      <div className="flex items-center gap-4">
        <div className="text-[#0078d4]">{icon}</div>
        <div>
          <h4 className="font-medium text-white">{title}</h4>
          <p className="text-sm text-gray-500">{description}</p>
        </div>
      </div>
      {children}
    </div>
  );
}
