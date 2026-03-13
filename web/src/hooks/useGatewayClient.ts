import { useEffect, useRef, useState } from "react";
import { GatewayClient, type HelloOk } from "../lib/gateway-client";

export function useGatewayClient() {
  const clientRef = useRef<GatewayClient | null>(null);
  const [connected, setConnected] = useState(false);
  const [helloOk, setHelloOk] = useState<HelloOk | null>(null);

  useEffect(() => {
    const client = new GatewayClient();
    clientRef.current = client;
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const url = `${protocol}//${window.location.host}/ws/__dashboard__`;

    client.connect(url).then((hello) => {
      setConnected(true);
      setHelloOk(hello);
    }).catch((err) => {
      console.error("GatewayClient connect failed:", err);
    });

    return () => {
      client.close();
      setConnected(false);
    };
  }, []);

  return { client: clientRef.current, connected, helloOk };
}
