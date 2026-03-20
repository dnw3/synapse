import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    proxy: {
      "/api": "http://localhost:3000",
      "/ws/gateway": {
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
