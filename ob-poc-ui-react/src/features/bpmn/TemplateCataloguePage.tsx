import { useCallback, useEffect, useState } from "react";
import { bpmnApi } from "@/api/bpmn";
import type { PublishedTemplateSummary } from "@/api/bpmn";

export function TemplateCataloguePage() {
  const [templates, setTemplates] = useState<PublishedTemplateSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [spawning, setSpawning] = useState<string | null>(null);
  const [spawnResults, setSpawnResults] = useState<Record<string, string>>({});
  const [spawnErrors, setSpawnErrors] = useState<Record<string, string>>({});

  const refresh = useCallback(() => {
    setLoading(true);
    bpmnApi
      .listPublishedTemplates()
      .then((list) => {
        setTemplates(list);
        setError(null);
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handleSpawn = async (tpl: PublishedTemplateSummary) => {
    const key = `${tpl.template_key}:${tpl.template_version}`;
    setSpawning(key);
    setSpawnErrors((prev) => ({ ...prev, [key]: "" }));
    try {
      const result = await bpmnApi.spawnFromTemplate(tpl.template_key, tpl.template_version);
      setSpawnResults((prev) => ({ ...prev, [key]: result.instance_id }));
    } catch (e) {
      setSpawnErrors((prev) => ({ ...prev, [key]: String(e) }));
    } finally {
      setSpawning(null);
    }
  };

  return (
    <div className="flex h-full flex-col overflow-hidden bg-gray-950 text-gray-100">
      <div className="border-b border-gray-800 p-4">
        <h1 className="text-sm font-semibold text-gray-200">Template Catalogue</h1>
        <p className="mt-1 text-xs text-gray-500">
          Published workflow templates — pick one to spawn a runnable instance.
        </p>
      </div>

      <div className="flex-1 overflow-y-auto p-4">
        {loading && <div className="text-xs text-gray-500">Loading…</div>}
        {error && (
          <div className="text-xs text-red-400">
            Failed to load templates: {error}
          </div>
        )}
        {!loading && !error && templates.length === 0 && (
          <div className="text-xs text-gray-600">
            No published templates yet — save a design session as a template
            in the designer first.
          </div>
        )}

        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {templates.map((tpl) => {
            const key = `${tpl.template_key}:${tpl.template_version}`;
            return (
              <div
                key={key}
                className="rounded border border-gray-800 bg-gray-900 p-3"
              >
                <div className="font-mono text-sm text-gray-100">
                  {tpl.template_key}
                </div>
                <div className="mt-0.5 text-xs text-gray-500">
                  v{tpl.template_version} · {tpl.process_key}
                </div>
                {tpl.task_manifest.length > 0 && (
                  <div className="mt-2 flex flex-wrap gap-1">
                    {tpl.task_manifest.map((t) => (
                      <span
                        key={t}
                        className="rounded bg-gray-800 px-1.5 py-0.5 text-[10px] font-mono text-gray-400"
                      >
                        {t}
                      </span>
                    ))}
                  </div>
                )}

                <button
                  onClick={() => handleSpawn(tpl)}
                  disabled={spawning === key}
                  className="mt-3 w-full rounded bg-blue-900 px-2 py-1.5 text-left text-xs font-mono text-blue-100 transition-colors hover:bg-blue-800 disabled:opacity-50"
                >
                  {spawning === key ? "Spawning…" : "▶ Spawn instance"}
                </button>

                {spawnResults[key] && (
                  <div className="mt-2 truncate text-xs text-green-400">
                    instance_id: {spawnResults[key]}
                  </div>
                )}
                {spawnErrors[key] && (
                  <div className="mt-2 text-xs text-red-400">
                    {spawnErrors[key]}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>

      <div className="border-t border-gray-800 p-2">
        <button
          onClick={refresh}
          className="w-full py-1 text-xs text-gray-500 transition-colors hover:text-gray-300"
        >
          Refresh
        </button>
      </div>
    </div>
  );
}
