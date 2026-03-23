import { createContext, useContext } from "react";
import type { UseGatewayReturn } from "./hooks/useGateway";
import type { UseSessionReturn } from "./hooks/useSession";
import type { IdentityInfo } from "./types/dashboard";

// ---------------------------------------------------------------------------
// Contexts shared across the app — extracted to avoid Fast Refresh warnings
// ---------------------------------------------------------------------------

export const IdentityContext = createContext<IdentityInfo | null>(null);
export const useIdentity = () => useContext(IdentityContext);

export const GatewayContext = createContext<UseGatewayReturn | null>(null);
export const useGw = () => {
  const ctx = useContext(GatewayContext);
  if (!ctx) throw new Error("useGw must be used within AppShell");
  return ctx;
};

export const SessionContext = createContext<UseSessionReturn | null>(null);
export const useSessionCtx = () => {
  const ctx = useContext(SessionContext);
  if (!ctx) throw new Error("useSessionCtx must be used within AppShell");
  return ctx;
};
