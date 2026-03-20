import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    hmr: {
      // Use a distinct path for Vite HMR WebSocket to avoid conflict with /ws proxy
      path: "/__vite_hmr",
    },
    proxy: {
      "/api": "http://localhost:3000",
      "/ws": {
        target: "ws://localhost:3000",
        ws: true,
        configure: (proxy) => {
          proxy.on("error", () => {}); // Suppress EPIPE on backend restart
        },
      },
    },
  },
  build: {
    outDir: "dist",
  },
});
