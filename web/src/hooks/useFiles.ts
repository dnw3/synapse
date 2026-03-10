import { useCallback, useState } from "react";
import type { FileEntry } from "../types";
import { api } from "../api";

export function useFiles(rootPath = ".") {
  const [currentPath, setCurrentPath] = useState(rootPath);
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState<string>("");
  const [loading, setLoading] = useState(false);

  const loadDirectory = useCallback(async (path: string) => {
    setLoading(true);
    try {
      const files = await api.listFiles(path);
      setEntries(files);
      setCurrentPath(path);
    } catch (e) {
      console.error("Failed to load directory:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  const openFile = useCallback(async (path: string) => {
    setLoading(true);
    try {
      const file = await api.readFile(path);
      setFileContent(file.content);
      setSelectedFile(path);
    } catch (e) {
      console.error("Failed to read file:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  const navigateUp = useCallback(() => {
    if (currentPath === "." || currentPath === "") return;
    const parts = currentPath.split("/").filter(Boolean);
    parts.pop();
    loadDirectory(parts.length > 0 ? parts.join("/") : ".");
  }, [currentPath, loadDirectory]);

  return {
    currentPath,
    entries,
    selectedFile,
    fileContent,
    loading,
    loadDirectory,
    openFile,
    navigateUp,
    setSelectedFile,
  };
}
