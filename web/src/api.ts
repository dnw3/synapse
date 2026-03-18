import type { Conversation, Message, FileEntry, FileContent, FileAttachment } from "./types";

const BASE = "/api";

async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  const res = await fetch(url, init);
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`${res.status}: ${text}`);
  }
  return res.json();
}

export const api = {
  // Conversations
  async createConversation(): Promise<Conversation> {
    return fetchJson(`${BASE}/conversations`, { method: "POST" });
  },

  async listConversations(): Promise<Conversation[]> {
    return fetchJson(`${BASE}/conversations`);
  },

  async getConversation(id: string): Promise<Conversation> {
    return fetchJson(`${BASE}/conversations/${id}`);
  },

  async deleteConversation(id: string): Promise<void> {
    await fetch(`${BASE}/conversations/${id}`, { method: "DELETE" });
  },

  // Messages (read-only — sending goes through WebSocket)
  async getMessages(conversationId: string): Promise<Message[]> {
    return fetchJson(`${BASE}/conversations/${conversationId}/messages`);
  },

  // Files
  async listFiles(path: string): Promise<FileEntry[]> {
    return fetchJson(`${BASE}/files?path=${encodeURIComponent(path)}`);
  },

  async readFile(path: string): Promise<FileContent> {
    return fetchJson(
      `${BASE}/files/content?path=${encodeURIComponent(path)}`
    );
  },

  async writeFile(path: string, content: string): Promise<void> {
    await fetch(`${BASE}/files/content?path=${encodeURIComponent(path)}`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ content }),
    });
  },

  async deleteFile(path: string): Promise<void> {
    await fetch(`${BASE}/files?path=${encodeURIComponent(path)}`, {
      method: "DELETE",
    });
  },

  // Uploads
  async uploadFile(file: File): Promise<FileAttachment & { size: number }> {
    const formData = new FormData();
    formData.append("file", file);
    return fetchJson(`${BASE}/upload`, {
      method: "POST",
      body: formData,
    });
  },
};
