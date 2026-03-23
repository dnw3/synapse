import { useEffect, useRef, useState } from "react";
import { GatewayClient, type HelloOk } from "../lib/gateway-client";

export function useGatewayClient() {
  const clientRef = useRef<GatewayClient | null>(null);
  const [client, setClient] = useState<GatewayClient | null>(null);
  const [connected, setConnected] = useState(false);
  const [helloOk, setHelloOk] = useState<HelloOk | null>(null);

  useEffect(() => {
    const gc = new GatewayClient();
    clientRef.current = gc;
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setClient(gc);
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const url = `${protocol}//${window.location.host}/ws/__dashboard__`;

    gc.connect(url).then((hello) => {
      setConnected(true);
      setHelloOk(hello);
    }).catch((err) => {
      console.error("GatewayClient connect failed:", err);
    });

    return () => {
      gc.close();
      setClient(null);
      setConnected(false);
    };
  }, []);

  return { client, connected, helloOk };
}
