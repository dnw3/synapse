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
          // Suppress EPIPE / ECONNRESET noise from both client and backend
          // sockets when either side disconnects unexpectedly.
          proxy.on("error", () => {});
          proxy.on("proxyReqWs", (_proxyReq, _req, socket) => {
            socket.on("error", () => {});
          });
          proxy.on("open", (proxySocket) => {
            proxySocket.on("error", () => {});
          });
        },
      },
    },
  },
  build: {
    outDir: "dist",
  },
});
